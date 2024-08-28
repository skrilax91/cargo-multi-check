mod cache;
mod config;

use crate::cache::{read_cache, write_cache};
use crate::config::{Config, GlobalConfig};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{exit, Command, ExitStatus, Stdio};
use std::time::Duration;
use std::{env, io};

#[derive(Debug)]
struct CheckError {
    combination: Vec<String>,
    message: String,
}

#[derive(Clone)]
struct RustProject {
    hash: u64,
    configs: GlobalConfig,
    path: PathBuf,
    features: Vec<String>,
    extra_features: Vec<String>,
    dependencies: HashMap<String, Vec<String>>,
}

impl RustProject {
    fn new(path: &str, configs: &str, cargo: Option<&String>) -> io::Result<Self> {
        let full_path = Path::new(path).canonicalize()?;
        let cargo_toml = match cargo {
            Some(c) => Path::new(c).canonicalize()?,
            None => full_path.join("Cargo.toml"),
        };
        let configs = Config::new(configs)?;
        let global_config = configs.global.clone();

        let (features, extra) = categorize_features(configs);

        let all_features = features.iter().chain(extra.iter()).collect::<HashSet<_>>();

        let dependencies = extract_dependencies(&cargo_toml, all_features)?;
        let hash = hash_features(&features, &dependencies);
        Ok(Self {
            hash,
            configs: global_config,
            path: full_path,
            features,
            extra_features: extra,
            dependencies,
        })
    }
}

fn categorize_features(config: Config) -> (Vec<String>, Vec<String>) {
    let mut main_features = Vec::new();
    let mut extra_features = Vec::new();

    for (feature, details) in config.features {
        if details.strict {
            main_features.push(feature);
        } else {
            extra_features.push(feature);
        }
    }

    main_features.sort();
    extra_features.sort();
    (main_features, extra_features)
}

fn extract_dependencies(
    file_path: &PathBuf,
    features: HashSet<&String>,
) -> io::Result<HashMap<String, Vec<String>>> {
    let mut dependencies = HashMap::new();
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut in_features_section = false;

    for line in reader.lines() {
        let line = line?;
        if line.trim() == "[features]" {
            in_features_section = true;
            continue;
        }
        if in_features_section {
            if line.starts_with('[') {
                break;
            }
            if let Some(pos) = line.find('=') {
                let feature = line[..pos].trim().to_string();

                // Check if feature is in list of features
                if !features.contains(&feature) && feature != "default" {
                    // Skip if feature is not in list of features and warn user
                    eprintln!(
                        "Warning: Feature {} is not in list of tested features",
                        feature
                    );
                }

                let deps: Vec<String> = line[pos + 1..]
                    .trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .filter_map(|s| {
                        let trimmed = s.trim().trim_matches('"');
                        // Remove "
                        if trimmed.is_empty() || trimmed.starts_with("dep:") {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    })
                    .collect();
                dependencies.insert(feature, deps);
            }
        }
    }

    Ok(dependencies)
}

fn hash_features(features: &[String], dependencies: &HashMap<String, Vec<String>>) -> u64 {
    let mut hasher = DefaultHasher::new();
    features.hash(&mut hasher);
    for feature in features {
        if let Some(deps) = dependencies.get(feature) {
            deps.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn generate_combinations(project: &RustProject) -> Vec<Vec<String>> {
    let n = project.features.len();
    let pb = ProgressBar::new(((1 << n) * (project.extra_features.len() + 1)) as u64);
    let style = ProgressStyle::default_bar()
        .template("{bar:40.cyan/blue} {pos}/{len}")
        .unwrap()
        .progress_chars("#>-");
    pb.set_style(style);
    let mut combinations = Vec::new();
    for i in 1..(1 << n) {
        let mut combo = Vec::new();
        let mut include = HashSet::new();
        let mut exclude = HashSet::new();
        for j in 0..n {
            if i & (1 << j) != 0 {
                let feature = &project.features[j];
                if !exclude.contains(feature) {
                    combo.push(feature.clone());
                    include.insert(feature.clone());
                    if let Some(deps) = project.dependencies.get(feature) {
                        for dep in deps {
                            exclude.insert(dep.clone());
                        }
                    }
                }
            }
        }
        let filtered_combo: Vec<String> =
            combo.into_iter().filter(|f| !exclude.contains(f)).collect();
        if !filtered_combo.is_empty() {
            for extra in &project.extra_features {
                let mut extended_combo = filtered_combo.clone();
                extended_combo.push(extra.clone());
                combinations.push(extended_combo);
                pb.inc(1);
            }
            combinations.push(filtered_combo);

            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{bar:40.green} {pos}/{len}")
                    .unwrap(),
            );
        } else {
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{bar:40.red} {pos}/{len}")
                    .unwrap(),
            );
        }

        pb.inc(1);
    }
    pb.finish();

    // Add extra features only
    for extra in &project.extra_features {
        combinations.push(vec![extra.clone()]);
    }
    combinations
}

async fn make_checks(
    combo: Vec<String>,
    path: &Path,
    check_pb: &ProgressBar,
    global_pb: &ProgressBar,
) -> Result<ExitStatus, (String, Vec<String>)> {
    let combo_str = combo.join(" ");

    let output = {
        if combo_str.is_empty() {
            check_pb.set_message("Running cargo check");
            Command::new("cargo")
                .current_dir(path)
                .arg("check")
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
                .wait_with_output()
                .unwrap()
        } else {
            check_pb.set_message(format!(
                "Running cargo check --no-default-features --features \"{}\"",
                combo_str
            ));
            Command::new("cargo")
                .current_dir(path)
                .arg("check")
                .arg("--no-default-features")
                .arg("--features")
                .arg(&combo_str)
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
                .wait_with_output()
                .unwrap()
        }
    };

    if output.status.success() {
        global_pb.inc(1);
        Ok(output.status)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        global_pb.inc(1);
        Err((stderr, combo))
    }
}

async fn run_cargo_build(project_dir: &Path, pb: &ProgressBar) -> io::Result<()> {
    pb.set_message("Fetching dependencies");
    let output = Command::new("cargo")
        .arg("build")
        .arg("--all-features")
        .current_dir(project_dir)
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap();

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(io::Error::new(io::ErrorKind::Other, stderr))
    }
}

async fn clear_terminal() {
    let status = Command::new("clear").status().unwrap();

    if !status.success() {
        eprintln!("Failed to clear terminal");
    }
}

async fn clear_project(project: &RustProject) -> Result<(), String> {
    let status = Command::new("cargo")
        .arg("clean")
        .current_dir(&project.path)
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .status()
        .unwrap();

    if !status.success() {
        Err("Failed to clean project".to_string())
    } else {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        println!("Please provide a rust project file path as an argument and a configuration file path as an argument");
        return;
    }

    if args[1] == "--help" {
        println!(
            "Usage: cargo run <path_to_cargo_toml> <path_to_toml_config> [cargo_toml_file_name]"
        );
        return;
    }

    let cargo = {
        if args.len() > 3 {
            Some(&args[3])
        } else {
            None
        }
    };

    let timer = std::time::Instant::now();
    let project =
        RustProject::new(&args[1], &args[2], cargo).expect("Failed to create Rust project");
    let cache_file = "feature_combinations.cache";

    if project.configs.clear_terminal {
        clear_terminal().await;
    }

    println!("Testing project: {:?}", project.path);
    println!("Using configuration file: {:?}", args[2]);
    println!("Setting concurrency to: {}", project.configs.concurrency);
    println!("--------------------------------------------------\n\n");
    println!("Found features: {:?}", project.features);
    println!("Found extra features: {:?}", project.extra_features);
    for (feature, dependencies) in &project.dependencies {
        if (dependencies.is_empty()) || dependencies == &[""] {
            continue;
        }
        println!("Feature: {} depends on {:?}", feature, dependencies);
    }

    // Calculer et afficher le nombre total de combinaisons
    let total_combinations =
        (1 << project.features.len()) * (project.extra_features.len() + 1) as u64;
    println!("Total possible combinations: {}", total_combinations);

    let cached_combinations = if Path::new(cache_file).exists() {
        let (cached_hash, cached_combinations) =
            read_cache(cache_file).expect("Failed to read cache");
        if project.hash == cached_hash {
            println!("Using cached combinations");
            cached_combinations
        } else {
            println!("Features have changed, regenerating combinations");
            let combinations = generate_combinations(&project);
            let unique_combinations: HashSet<Vec<String>> = combinations.into_iter().collect();
            write_cache(cache_file, project.hash, &unique_combinations)
                .expect("Failed to write cache");
            unique_combinations
        }
    } else {
        println!("No cache found, generating combinations");
        let combinations = generate_combinations(&project);
        let unique_combinations: HashSet<Vec<String>> = combinations.into_iter().collect();
        write_cache(cache_file, project.hash, &unique_combinations).expect("Failed to write cache");
        unique_combinations
    };

    println!("Total unique combinations: {}", cached_combinations.len());

    if project.configs.clean {
        let clean_spinner = ProgressBar::new_spinner();
        clean_spinner.set_style(
            ProgressStyle::default_spinner()
                .template("[{elapsed_precise}]{spinner:.green} {msg}")
                .unwrap(),
        );
        clean_spinner.enable_steady_tick(Duration::from_millis(100));
        clean_spinner.set_message("Cleaning project");
        match clear_project(&project).await {
            Ok(_) => clean_spinner.finish_with_message("Project cleaned successfully"),
            Err(_) => {
                clean_spinner.finish_with_message("Failed to clean project");
                exit(1);
            }
        }
    }

    {
        let build_spinner = ProgressBar::new_spinner();
        build_spinner.set_style(
            ProgressStyle::default_spinner()
                .template("[{elapsed_precise}]{spinner:.green} {msg}")
                .unwrap(),
        );
        build_spinner.enable_steady_tick(Duration::from_millis(100));
        build_spinner.set_message("Building project for testing");
        run_cargo_build(&project.path, &ProgressBar::hidden())
            .await
            .expect("Failed to build project");
        build_spinner.finish_with_message("Project built successfully");
    }

    let multi_progress = MultiProgress::new();
    let mut progresses = vec![];
    let mut handles = vec![];

    // Add spinners
    for i in 0..project.configs.concurrency {
        let spinner = multi_progress.add(ProgressBar::new_spinner());
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template(
                    format!(
                        "[{}/{}] {{spinner:.green}} {{msg}}",
                        i % project.configs.concurrency + 1,
                        project.configs.concurrency
                    )
                    .as_str(),
                )
                .unwrap(),
        );
        spinner.enable_steady_tick(Duration::from_millis(100));
        progresses.push(spinner);
    }

    let global_progress = multi_progress.add(ProgressBar::new(cached_combinations.len() as u64));
    global_progress.enable_steady_tick(Duration::from_millis(100));
    global_progress.set_style(ProgressStyle::default_bar().template("[{elapsed_precise}] {wide_bar:0.cyan/blue} Tested {pos}/{len} ({percent}%) | remaining: {eta_precise}").unwrap());
    for (i, combo) in cached_combinations.into_iter().enumerate() {
        let path_clone = project.path.clone();
        let pb = progresses[i % project.configs.concurrency].clone();
        let gl_pb = global_progress.clone();
        let handle =
            tokio::spawn(async move { make_checks(combo, &path_clone, &pb, &gl_pb).await });
        handles.push(handle);
    }

    let mut fail_list = vec![];

    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => (),
            Err((error, combination)) => {
                fail_list.push(CheckError {
                    combination,
                    message: error,
                });
            }
        }
    }

    multi_progress.clear().unwrap();

    if project.configs.clear_terminal {
        clear_terminal().await;
    }

    if fail_list.is_empty() {
        println!("All checks passed");
        println!("Done in {:?}", timer.elapsed());
    } else {
        println!("{:?} checks failed", fail_list.len());
        for fail in fail_list {
            println!("\nFailed combination: {:?}", fail.combination.join(" "));
            println!("Error: {}", fail.message);
            println!("----------------------");
        }

        println!("Done in {:?}", timer.elapsed());
        exit(1);
    }
}
