# Opossum

![OPOSSUM Logo](https://raw.githubusercontent.com/opossum-labs/opossum/refs/heads/main/opossum_core/logo/Logo_text.svg)

OPOSSUM is an advanced simulation platform for optical systems, with a special focus on designing and analyzing large-scale laser systems.

## 🚀 Core Concepts

At its heart, OPOSSUM represents optical systems as a graph:

* **Graph-Based Model:** The system is a collection of nodes, where each node represents an optical component.
* **Hierarchical Systems:** Components can be grouped into subgraphs, allowing for complex, nested designs.
* **Modular Analysis:** The model can be processed by various analyzer modules (e.g., simple energy flow analysis, ray tracing, or ghost focus analysis).
* **Detailed Reporting:** Each analysis module generates a specific report based on its findings.

## 📦 The OPOSSUM Suite

The project is broken down into several key components:

* `opossum_core`: The core library (written in **Rust** 🦀) containing the graph model, component node types, and analyzer modules.
* `opossum_cli`: A command-line interface that uses the core library to run an analysis from an OPOSSUM model file.
* `opossum_backend`: A REST API server (also using `opossum_core`) that acts as the main communication hub for the graphical interface.
* `opossum_gui`: The graphical frontend. It communicates with `opossum_backend` and uses `opossum_cli` to execute the actual simulation.

## ⚠️ Project Status

> **This project is in an early design and development phase.**
>
> It should not be used in a productive environment. APIs, core functions, file formats, and concepts are subject to change without notice.

## 🛠️ Building from Source

1. Make sure you have a working rust development environment (see [rustup](http://rustup.rs) to get started).
2. Clone the repository:

   ```bash
   git clone [https://github.com/opossum_labs/opossum.git](https://github.com/opossum_labs/opossum.git)
   cd opossum
   ```

3. Build the CLI:

   ```bash
   cd opossum_cli
   cargo build
   ```

4. Build the backend:

   ```bash
   cd opossum_backend
   cargo run
   ```

5. Build and start the GUI:

   ```bash
   cd opossum_gui # in a new terminal
   dx serve
   ```

## 🤝 Contributing

Contributions are what make the open-source community such an amazing place to learn, inspire, and create. We welcome contributions, and we are excited to see what you bring!

If you have an idea for an enhancement or have found a bug, please don't hesitate to open an issue.

### How to Contribute

We use the standard GitHub "Fork & Pull Request" workflow.

1. **Fork** the repository to your own GitHub account.
2. **Clone** your fork to your local machine:
3. **Create a new branch** for your changes. Please use a descriptive name:
4. **Make your changes** and **commit** them.
    * Please write clear and descriptive commit messages.
    * If your changes fix a specific issue, mention it in the commit message (e.g., `Fixes #123`).
5. **Run tests and linters** to ensure everything is still working correctly.
6. **Push** your branch to your fork on GitHub:
7. **Open a Pull Request** (PR) from your branch to the `main` branch of the `opossum-labs/opossum` repository.
8. **Describe your changes** in the PR. Explain *what* you changed and *why*. Link to any relevant issues.

### Contribution Guidelines

* **Bug Reports:** Please use the "Bug Report" issue template and provide as much detail as possible, including steps to reproduce the bug.
* **Feature Requests:** Use the "Feature Request" issue template. Please describe the problem you're solving and your proposed solution. This allows for discussion before you spend time on implementation.
* **Code Style:** This project uses `rustfmt` for code formatting. Please run `cargo fmt` before committing your changes.
* **Unit Tests** If possible, add unit tests which proving either that a bug has been fixed or a new feature works as expected. This keeps the code quality high and ensures following contributions by others do not break your contributions.

### License

By contributing, you agree that your contributions will be licensed under the project's [LICENSE-GPLv3](LICENSE) license.

## 📄 License

OPOSSUM is published under a [GPLv3](LICENSE) license.
