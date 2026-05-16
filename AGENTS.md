# Agent Guide: djlintr

This document provides guidance for AI agents on how to develop, test, and release `djlintr`.

## Project Overview

`djlintr` is a fast HTML template linter and formatter, ported from the Python tool `djlint` to Rust. It aims for high performance and compatibility with the original tool.

## Development Workflow

### Building the Project

You can build the project using standard Cargo commands or the provided `Makefile`.

```bash
# Debug build
make build
# or
cargo build

# Release build
make release
# or
cargo build --release
```

### Running the CLI

```bash
cargo run -- [OPTIONS] <PATHS>...
```

### Testing

Always run tests before submitting changes.

```bash
# Run all tests
make test
# or
cargo test

# Run a specific test file
cargo test --test <test_name>
```

### Linting and Formatting

Ensure the code adheres to Rust standards. **You MUST NOT consider a task finished until `make lint` passes.**

```bash
# Run clippy and check formatting (used in CI)
make lint

# Apply formatting
make fmt
```

## Parity Testing

A key goal is parity with the original Python `djlint`. There is a specialized setup for comparing lint results.

1.  **Install djlint in a venv:**
    ```bash
    make install-djlint
    ```
2.  **Fetch test data:**
    ```bash
    make fetch-test-data
    ```
3.  **Run parity comparison:**
    ```bash
    make compare-lint
    ```
    This script compares the output of `djlintr` against `djlint` on a set of templates.

## Release Process

We use `cargo-release` for managing versions and tags.

1.  **Preparation:**
    - Ensure you are on the `main` branch.
    - Ensure all tests, lint, and parity checks pass.
    - Check that `CHANGELOG.md` is updated.

2.  **Execute Release:**
    Run `cargo release` to bump the version, create a git tag, and push to the repository.
    **Note:** `consolidate-commits` must be set to `false` in `Cargo.toml` for `{{version}}` placeholders to render correctly in commit messages.
    ```bash
    # Dry run
    cargo release <patch|minor|major> --execute --dry-run
    
    # Real release
    cargo release <patch|minor|major> --execute
    ```

3.  **CI Automation:**
    Once a tag (e.g., `v0.5.2`) is pushed, the GitHub Actions workflow (`.github/workflows/ci.yml`) will:
    - Run the full test suite.
    - Build release binaries for Linux (x64), macOS (x64, ARM64), and Windows (x64).
    - Create a new GitHub Release and upload the binaries.

## Architectural Notes

- **Linter Rules:** Located in `src/linter/mod.rs`.
- **Formatter Logic:** Located in `src/formatter/`.
- **Configuration:** Handled in `src/config.rs`, supporting `.djlintrc` and `pyproject.toml`.
- **Parallelism:** Uses `rayon` for fast file processing.
