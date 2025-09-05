# opossum_core

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

> The core library for **OPOSSUM**, the **O**pen **s**ource **o**ptic **s**imulation **s**ystem and **u**nified **m**odeler.

## About

`opossum_core` provides the foundational components for building and simulating optical systems in Rust. It is designed to be used directly in your own Rust projects or as the engine for the other tools in the OPOSSUM ecosystem:

* **`opossum-cli`**: A command-line interface for analyzing models.
* **`opossum-backend`**: A web API backend.
* **`opossum-gui`**: A graphical user interface for modeling and simulation.

This crate contains everything you need to define optical components, trace rays, and analyze system performance.

## Getting Started

To use `opossum_core` in your own project, you can add it as a dependency in your `Cargo.toml` file.

### Prerequisites

Ensure you have a recent version of the Rust toolchain installed. If not, you can install it using [rustup](https://rustup.rs/).

### Installation

Add the following line to your `Cargo.toml`:

```toml
[dependencies]
opossum_core = "0.1.0" # Replace with the latest version
```

### Usage

Here is a brief example of how you might use opossum_core to define a simple optical system.

```rust
use opossum_core::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    millimeter,
    nodes::{Dummy, NodeGroup},
};
use std::path::Path;

fn main() -> OpmResult<()> {
    // Create a new model
    let mut scenery = NodeGroup::new("OpticScenery demo");
    // Add two optical nodes
    let node1 = scenery.add_node(Dummy::new("dummy1"))?;
    let node2 = scenery.add_node(Dummy::new("dummy2"))?;
    // Connect the nodes
    scenery.connect_nodes(node1, "output_1", node2, "input_1", millimeter!(0.0))?;

    // Create a model document
    let mut doc = OpmDocument::new(scenery);
    // Add a ray tracing analyzer.
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    // Save the model as .opm file
    doc.save_to_file(Path::new("./opticscenery.opm"))
    // The generated file opticscenery.opm can now be used as input e.g. for the command line tool to
    // generate an analysis report.
}
```

For more detailed examples, check out the `examples` directory in this repository. Most examples produce an OPOSSUM model file (`*.opm`), which can be analyzed with opossum-cli.

### Building from Source

If you have cloned this repository, you can build and test the library using standard cargo commands.

```bash
# Build the library in release mode
cargo build --release

# Run the test suite
cargo test
```

## Documentation

To build the full API documentation, including doc-images, run the following command:

```bash
cargo doc --no-deps --features doc-images
```

After running, open the documentation in your browser at target/doc/opossum_core/index.html.

## License

This project is licensed under the GNU General Public License v3.0. See the LICENSE file for the full license text.
