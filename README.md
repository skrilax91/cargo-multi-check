# Cargo Feature Tester

Cargo Feature Tester is a Rust project designed to test all possible combinations of Cargo features against different configurations using `cargo check`. This project is useful for ensuring that all feature combinations of a Rust crate compile successfully with various configuration setups.

## Features

- **Automated Testing**: Automatically tests all possible combinations of features defined in your `Cargo.toml`.
- **Configuration Management**: Supports custom configuration setups defined in a TOML file.
- **Caching**: Utilizes caching to avoid redundant checks and improve performance.

## Installation

To use Cargo Feature Tester, you'll need to have Rust and Cargo installed. You can install Rust using `rustup`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Clone the repository and navigate into the project directory:

```bash
git clone <repository-url>
cd cargo-feature-tester
```

## Configuration

The project requires a configuration file (`Configs.toml`) to define the setups and options for testing. The configuration is divided into two main sections: `global` and `features`.

### Global Section

The `global` section is used to configure the script's behavior with the following keys:

- `concurrency`: Specifies the number of checks to run in parallel.
- `clean`: Indicates whether to execute a `cargo clean` before starting the tests. Accepts boolean values (`true` or `false`).
- `clear_terminal`: If set to `true`, the program will execute a `clear` command on the console before and after execution.

Example:

```toml
[global]
concurrency = 4
clean = true
clear_terminal = false
```

### Features Section

The `features` section lists all the features to be tested. For each feature, the following options are available:

- `strict`: If set to `true`, this feature will be tested with all other features. If set to `false`, it will only be tested with other strict features.

Example:

```toml
[features]
feature1 = { strict = true }
feature2 = { strict = false }
feature3 = { strict = true }
feature4 = { strict = false }
```

In this example, `feature1` and `feature3` will be tested in combination with all other features, while `feature2` and `feature4` will only be tested with other features marked as `strict`. A test with feature2 and feature4 will never be done.

## Usage

To run the tests for all feature combinations, use the following command:

```bash
cargo run <path-to-Cargo.toml> <path-to-Configs.toml>
```

This will execute `cargo check` for each combination of features defined in your `Cargo.toml` file, according to the configurations specified in `Configs.toml`.

## Structure

- `src/main.rs`: The main entry point of the application.
- `src/config.rs`: Handles reading and parsing of the configuration file.
- `src/cache.rs`: Manages caching of test results to optimize performance.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any enhancements or bug fixes.


## Acknowledgements

Thank you for using Cargo Feature Tester. We hope it helps streamline your Rust development process.