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

## 📚 Building the OPOSSUM Documentation (Optional)

The **OPOSSUM suite** has three main sources of documentation:

1. **The Book:** The comprehensive guide you are currently reading.
2. **Library API Documentation:** For the core Rust library.
3. **REST API Documentation:** For the backend server interface.

---

### 1. 📖 Build the Documentation Book

The book is built using [`mdBook`](https://rust-lang.github.io/mdBook/). This system compiles the Markdown source files into a static, readable website.

1. **Install `mdBook`:**

   ```bash
   cargo binstall mdbook
   ```

2. **Navigate and Build:**

    ```bash
    cd opossum/doc/book
    mdbook build
    ```

The generated documentation website is located at: **`<target dir>/doc/book/book/index.html`**. You can open this file directly in any web browser.

---

### 2. 🦀 Build the Library API Documentation

The API documentation for the Rust core library is generated using **`rustdoc`**, the standard tool for Rust projects.

1. **Navigate to the Core Library:**

    ```bash
    cd opossum_core
    ```

2. **Generate Documentation:**

    ```bash
    cargo doc --no-deps --features "doc-images"
    ```

    > *The `--no-deps` flag speeds up the process by excluding documentation for dependencies.*

The resulting HTML documentation can be found in your target directory, typically at:
**`<target dir>/doc/opossum_core/index.html`**

---

### 3. 🌐 Access the REST API Documentation

The documentation for the backend's REST API is **automatically served** by the backend application itself when it is running.

1. Ensure the **backend server is running** (e.g., in a separate terminal).
2. Open the following URL in your web browser:
    [`http://localhost:8001`](http://localhost:8001)

Once the page loads, click the **`View API Documentation`** button to see the interactive Swagger UI.
