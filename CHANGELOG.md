# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Performance

- **Tokenizer**: Wrapped all regular expressions in the Tokenizer inside `OnceLock` statics to prevent recompiling 6 regular expressions from scratch on every single tokenization pass. This resulted in a massive 50% performance improvement across both formatting and linting.
- **Linter**: Re-architected template tag masking (`mask_template_tags`) to use `Cow<str>` and only allocate when template tags actually exist within the HTML tag, avoiding two regex passes and a string allocation for the vast majority of tokens.
- **Linter**: Short-circuited the full-text `djlint:off` regex scanners to run only if the document contains `"djlint:off"`, saving up to 7 full-text regex iterations per file in typical codebases.
- **Linter**: Prevented unconditional `to_lowercase()` string allocations for tag names and attributes, using zero-allocation `eq_ignore_ascii_case` or restricting allocations to only the specific tags that strictly require case-insensitive attribute searching (like `<img>` and `<meta>`). Together, these optimizations drop the `lint_large_template` benchmark execution time by ~50% (from ~44ms down to ~21ms).

## [0.5.12] - 2026-05-24

### Added

- **Python Packaging**: Added `maturin` configuration and a PyPI release GitHub action. `djlintr` can now be installed as a Python package via `pip install djlintr` or `uv pip install djlintr`.

### Fixed

- **CLI**: Removed leftover debug prints from the formatter.

## [0.5.8] - 2026-05-16

### Fixed

- **Tokenizer**: Improved tag identification to correctly handle quoted attribute values containing `>` characters (e.g. Alpine.js arrow functions), ensuring parity with `djlint` and preventing false-positive `H012` errors.

## [0.5.7] - 2026-05-16

### Fixed

- **H037 (Duplicate Attribute)**: Refined duplicate attribute detection to achieve full parity with `djlint`.

## [0.5.3] - 2026-05-16

### Fixed

- **H014 (Blank Lines)**: Strip newlines from reported match string to prevent extra blank lines in CLI output.

## [0.5.1] - 2026-05-16

### Fixed
- **100% Parity Reached**: Reproduced a specific `djlint` regex quirk for `H037` (duplicate attributes) that allowed it to jump across tags when nested quotes are present in template tags.
- **H014 (Blank Lines)**: Refined regex and ignored-block logic to match `djlint` exactly.
- **H030/H031 (Document Rules)**: Implemented a "commented out html" parity hack to match `djlint`'s document-level rule suppression.

## [0.5.0] - 2026-05-16

### Added
- **Ignored Blocks Logic**: Implemented `ignored_ranges` to skip linting inside `<script>`, `<style>`, `<pre>`, and `<textarea>` tags, as well as HTML comments, matching `djlint`'s behavior.
- **CLI & Config Alignment**: Added support for `--include` and `--ignore` flags.
- **New Regression Test Suite**: Added `tests/parity_regressions.rs` covering major fixes and behavioral changes.

### Changed
- **Tokenizer & Offsets**: Updated the `Tokenizer` to include the byte `offset` for each token, improving lint error location accuracy.
- **H008 (Double Quotes)**: Restricted to specific attributes (`class`, `id`, `src`, etc.) to match `djlint`.
- **H010 (Lowercase Attributes)**: Now only checks actual attribute names, ignoring uppercase content within values.
- **H020 (Empty Tag Pairs)**: Skips whitespace between tags and requires the opening tag to have no attributes.
- **H025 (Orphan Tags)**: Replaced simple counter with a robust stack-based check for nested orphans.
- **H014 (Blank Lines)**: Improved masking of template tags before regex matching for better parity.
- **Default Rules**: Aligned rules disabled by default with `djlint` (`H017`, `H035`, `H036`, `H030`, `H031`).

## [0.4.1] - 2026-05-12

### Fixed
- Internal lint error (clippy manual-pattern-char-comparison) in `src/linter/mod.rs`.

## [0.4.0] - 2026-05-10

### Added
- Profile support (`--profile`) to match Python djlint's rule exclusion logic. Supported profiles: `html`, `django`, `jinja`, `nunjucks`, `handlebars`, `golang`, `angular`, and `all`.
- Default profile is `html`, which correctly excludes template-specific rules (`T`, `D`, `J`, etc.) by default, achieving parity with Python djlint.

### Changed
- Simplified `T003` (named endblocks) implementation to match Python's regex exactly, removing the incorrect one-liner exception.

## [0.3.0] - 2026-05-10

### Added
- Support for `max-attribute-length` configuration option and CLI flag to control attribute wrapping.

## [0.2.0] - 2026-05-10

### Added
- Support for `custom_blocks` configuration option and CLI flag.
- Support for re-indenting Django/Jinja tags: `{% else %}`, `{% elif %}`, and `{% empty %}`.
- New `LICENSE` file (BSD 3-Clause).
- New `README.md` with installation and usage instructions.
- New `CHANGELOG.md` to track project changes.

### Changed
- Improved Django/Jinja tag indentation logic to only indent known block tags or user-defined custom blocks, preventing incorrect indentation for self-closing tags like `{% url %}`.
- Updated project version to v0.2.0.

## [0.1.1] - 2026-05-10

### Added
- Basic HTML/Template tokenizer.
- Basic indentation logic.
- Initial linter rule engine with several HTML and Template rules.
- Parallel file processing support using `rayon`.
- CLI implementation using `clap`.
- Configuration support for `.djlintrc` and `pyproject.toml`.

## [0.1.0] - 2026-05-09

### Added
- Initial project setup and infrastructure.
