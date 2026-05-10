# djlintr

A fast HTML template linter and formatter. Port of
[djlint](https://github.com/djlint/djLint) to Rust.

Currently only supports HTML and Django templates; Jinja2 and Nunjucks
to be added in the future.

## Installation

Just grab one of the binary release files from GitHub.

Or, to install `djlintr` from source, ensure you have Rust installed and run:

```bash
cargo install --path .
```

## Usage

```bash
djlintr [OPTIONS] <PATHS>...
```

### Arguments
* `<PATHS>...`: Files or directories to process.

### Options
* `-r, --reformat`: Reformat files.
* `-l, --lint`: Lint files.
* `-e, --extension <EXTENSION>`: File extensions to include (default: `html`).
* `-t, --threads <THREADS>`: Number of threads to use.
* `--check`: Return non-zero exit code if issues are found.
* `--max-attribute-length <MAX_ATTRIBUTE_LENGTH>`: Maximum length for attributes before wrapping (default: `70`).
* `--custom-blocks <CUSTOM_BLOCKS>`: Comma-separated list of custom block tags.
* `-h, --help`: Print help.
* `-V, --version`: Print version.

## Configuration

`djlintr` automatically searches for a `.djlintrc` (JSON) file or a `pyproject.toml` file in the current directory.

### Supported Options
* `indent`: Number of spaces for indentation (default: `4`).
* `max_line_length`: Maximum line length (default: `120`).
* `max_attribute_length`: Maximum length for attributes before wrapping (default: `70`).
* `ignore`: A list of rule codes to ignore.
* `custom_blocks`: A list of custom block tags.

#### Example `.djlintrc`
```json
{
  "indent": 2,
  "max_line_length": 100,
  "ignore": ["H006", "T001"]
}
```

#### Example `pyproject.toml`
```toml
[tool.djlint]
indent = 2
max_line_length = 100
ignore = ["H006", "T001"]
```
