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

Depending on your platform (Windows or Linux), the final installation packages will be generated in: `opossum/target/dx/bundle/opossum_gui/`.
