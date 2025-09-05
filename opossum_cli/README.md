# opossum_cli

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

> The command line interface for **OPOSSUM**, the **Open-source Optics Simulation System and Unified Modeler**, 

## About

`opossum_cli` provides a way to use the `opossum_core` library without the graphical user interface. 
It is part of the OPOSSUM ecosystem, among the tools:

* **`opossum-core`**: The core Rust library for optical simulation.
* **`opossum-backend`**: A web API backend.
* **`opossum-gui`**: A graphical user interface for modeling and simulation.

It allows loading optical setup files (`.opm`), analyzing models, and generating reports directly from the command line.  

---

## ✨ Features
- Run OPOSSUM completely without a GUI  
- Load **.opm** optical scenario files  
- Perform analysis and generate reports  
- ASCII-art logo and version info at startup  
- Input validation (file paths, report directories)  
- Automatically create reports in the chosen directory  

---

## 📦 Installation
The CLI is part of the Opossum framework and can be built via cargo directly from the project root:

```bash
cargo build --release
```

## 🚀 Usage
The CLI can be launched by running the generated executable or via the `cargo run` command in the project root.  
Several arguments can be passed.

| Flag / Option            | Description                                                                   |
| ------------------------ | ----------------------------------------------------------------------------- |
| `-f, --file-path`        | Path to the `.opm` file (required)                                            |
| `-r, --report-directory` | Destination directory for reports. Default: same directory as the `.opm` file |
| `-s, --show-logo`        | Show Opossum logo at startup (`true/false`, default: `true`)                  |
| `-h, --help`             | Show help                                                                     |
| `-V, --version`          | Show version information                                                      |

### Examples
To run actual examples, download one from the [repository](https://github.com/opossum-labs/opossum/tree/main/opossum_core/examples) and replace the paths and filenames below.

Analyze an optical setup and store reports in the same directory
```bash
cargo run -- -f ./your_path_to_an_opm_file/your_opm_file.opm
```

With a custom report directory
```bash
cargo run -- -f ./your_path_to_an_opm_file/your_opm_file.opm -r ./your_path_to_the_report_directory
```

Without the logo
```bash
cargo run -- -f ./your_path_to_an_opm_file/your_opm_file.opm -r ./your_path_to_the_report_directory -s false
```