# djlintr

A fast HTML template linter and formatter. Port of
[djlint](https://github.com/djlint/djLint) to Rust.

Currently only supports HTML and Django templates; Jinja2 and Nunjucks
to be added in the future.

## Installation

You can install `djlintr` directly from PyPI using `pip` or `uv`:

```bash
pip install djlintr
# or
uv pip install djlintr
```

Alternatively, you can grab one of the binary release files from GitHub.

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

### Python Compatibility Flags

`djlintr` aims for byte-for-byte parity with the original Python `djlint`. In a
couple of places djlintr has a "better" implementation than the Python original,
but defaults to reproducing the Python behaviour so that existing projects get
identical output. These flags let you opt in to the improved (but
intentionally non-identical) behaviour:

* `better_attribute_parsing` (default: `false`): When `false`, djlintr uses the
  same attribute-matching regex as Python `djlint`, quirks and all. Set to
  `true` to use djlintr's cleaner attribute-parsing regex, which handles some
  edge cases more correctly but may produce output that differs from Python
  `djlint`.
* `require_closed_blocks` (default: `false`): When `false`, djlintr indents the
  contents of a recognized block tag even when no explicit closing tag is
  present, matching Python `djlint`'s lenient behaviour. Set to `true` to only
  indent blocks that have an explicit closing tag.

#### Example `.djlintrc`
```json
{
  "indent": 2,
  "max_line_length": 100,
  "ignore": ["H006", "T001"]
}
```

#### Example `pyproject.toml`
`djlintr` supports both `[tool.djlint]` and `[tool.djlintr]` sections.
```toml
[tool.djlintr]
indent = 2
max_line_length = 100
ignore = ["H006", "T001"]
```
