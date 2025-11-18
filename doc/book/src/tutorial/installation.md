# Installation

There are two ways to install OPOSSUM: using the pre-built binary packages (recommended for most users) or building it from the source code.

## 1. Pre-built Packages (Recommended)

For the easiest installation, download the latest installer for your operating system from our GitHub releases page:

* **Download:** [OPOSSUM GitHub Releases](https://github.com/opossum-labs/opossum/releases)
* **Available Platforms:**
  * **Windows:** `.msi`, `.exe`
  * **Linux:** `.deb`, `.rpm`, `.AppImage`

Run the installer following the standard procedure for your operating system.

> **Recommendation:** To utilize the full potential of OPOSSUM, we strongly recommend installing **[Graphviz](https://graphviz.org)**.
>
> This tool is used to generate graphical representations of the optical graph model. OPOSSUM is fully functional without it, but analysis reports will not include diagrams.

## 2. Build from Source

To build OPOSSUM, you must have the **Rust** programming language installed. If you have not installed Rust yet, please follow the instructions on the [official Rust installation page](https://www.rust-lang.org/tools/install).

### Get the Code

Clone the repository to get the latest (potentially unstable) version, or download a stable source archive from the releases page.

```bash
git clone [https://github.com/opossum-labs/opossum.git](https://github.com/opossum-labs/opossum.git)
cd opossum
```

### Prerequisities

Before building, some tools need to be installed first. Since the OPOSSUM frontend GUI is based on the dioxus framework we need to install the dioxus CLI. The easiest
is to install the binary directly. For this, `cargo-binstall` must be installed first.

```bash
cargo install cargo-binstall
cargo binstall dioxus-cli
```

### Option A: Build Development Version

Use this method if you want to develop or debug OPOSSUM. The executables will be unoptimized (debug build), and the backend and frontend must be run in separate terminals.

### Step 1: Build the CLI

First, build the Command Line Interface tool.

```bash
cd opossum_cli
cargo build
cd ..
```

### Step 2: Run the Backend (Terminal 1)

The frontend requires the REST backend server to be running. In your current terminal, build and run the backend:

```bash
cd opossum_backend
cargo run
```

***Leave this terminal open.***

### Step 3: Run the GUI Frontend (Terminal 2)

Open a new terminal, navigate to the opossum directory, and start the Dioxus development server:

```bash
cd opossum_gui
dx serve
```

### Option B: Build Installation Packages

Use this method to create optimized release bundles. The components must be compiled separately.

### Step 1: Build CLI (Release)

```bash
cd opossum_cli
cargo build --release
cd ..
```

### Step 2: Build Backend (Release)

For bundling, the backend only needs to be compiled, not run.

```bash
cd opossum_backend
cargo build --release
cd ..
```

### Step 3: Bundle the GUI

Finally, run the bundling command.

```bash
cd opossum_gui
dx bundle --release --features "bundle-backend"
```

Depending on your platform (Windows or Linux), the final installation packages will be generated in: `<target dir>/dx/bundle/opossum_gui/`.

## 3. Build the documentation (optional)

The OPOSSUM suite has three locations of documentation:

* The book ... this is what you are currently reading `:-)`
* The library API documentation
* The REST API documentation of the backend server

### Build the book

The book uses [`mdbook`](https://rust-lang.github.io/mdBook/) as documentation system. A book can be compiled with the following commands:

```bash
cargo binstall mdbook
cd opossum/doc/book
mdbook build
```

The generated documentation can be found at `opossum/doc/book/book/index.hml` which can be opened in a web browser.

### Build the library API documentation

The API documentation uses the standard `rustdoc` system. To generate the documentation follow these steps:

```bash
cd opossum_core
cargo doc --no-deps --features "doc-images"
```

The resulting HTML documentation is found at `<target dir>/doc/opossum_core/index.html`.

### REST API backend documentation

The backend documentation is automatically provided the the backend server itself. If the backend server is running, use the following URL in your
browser: [`localhost:8001/`](http://localhost:8001) and click the `View API Documentation` button.
