use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::io::Write;
use std::io::{BufRead, BufReader};

pub fn read_cache(cache_file: &str) -> io::Result<(u64, HashSet<Vec<String>>)> {
    let file = File::open(cache_file)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let hash = lines.next().unwrap()?.parse::<u64>().unwrap();
    let mut combinations: HashSet<Vec<String>> = HashSet::new();

    for line in lines {
        let line = line?;
        let combo: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
        combinations.insert(combo);
    }

    Ok((hash, combinations))
}

pub fn write_cache(
    cache_file: &str,
    hash: u64,
    combinations: &HashSet<Vec<String>>,
) -> io::Result<()> {
    let mut file = File::create(cache_file)?;

    writeln!(file, "{}", hash)?;
    for combo in combinations {
        writeln!(file, "{}", combo.join(" "))?;
    }

    Ok(())
}
