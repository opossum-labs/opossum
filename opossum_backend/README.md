# opossum_backend

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

> The backend API server for **OPOSSUM**, the **O**pen **s**ource **o**ptic **s**imulation **s**ystem and **u**nified **m**odeler.

## About

`opossum_backend` is the REST API server for the **OPOSSUM** ecosystem, providing web access to the `opossum_core` simulation library. It is a key part of the project, which also includes:

* **`opossum_core`**: The core Rust library for optical simulation.
* **`opossum_cli`**: A command-line interface for analyzing models.
* **`opossum_gui`**: A graphical user interface that uses this backend for its operations.

While its primary role is to support `opossum_gui`, the API can be accessed by any application or programming language capable of making standard HTTP requests.

## Getting Started

You can get the server running by building it from source or installing it directly with `cargo`.

### Prerequisites

Ensure you have a recent version of the Rust toolchain installed. If not, you can install it using [rustup](https://rustup.rs/).

### Installation

There are two common ways to install and run the server:

1. **Build from Source (Recommended)**
    Clone the repository and build the release binary:

    ```bash
    cargo build --release
    ```

    The executable will be available at `./target/release/opossum_backend`.

2. **Install with Cargo**
    You can also install the binary directly from crates.io (once published):

    ```bash
    cargo install opossum_backend
    ```

    This will make the `opossum_backend` command available in your shell.

***

## Usage

First, start the server by running the executable:

```bash
# If you built from source
./target/release/opossum_backend

# If you installed with cargo
opossum_backend
```

By default, the API server will start on `http://localhost:8001`. You can visit this URL in your browser to see a simple landing page.
You can interact with the API using any HTTP client, like `curl`. E.g.

```bash
curl http://localhost:8001/api/version
```

```json
{"backend_version":"0.6.0","opossum_version":"0.6.0-370-gc43cb45 (2025/09/05 09:07)"}
```

## Documentation

The complete API is documented using the [OpenAPI 3.1 specification](https://en.wikipedia.org/wiki/OpenAPI_Specification).
Interactive documentation is available via SwaggerUI, which is hosted by the server itself. Once the server is running, you
can access it at: `http://localhost:8001/swagger-ui`

For a deeper understanding of the underlying data models, consulting the `opossum_core` library documentation is also recommended.

## License

This project is licensed under the GNU General Public License v3.0. See the LICENSE file for the full license text.
