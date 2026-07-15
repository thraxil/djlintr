# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.4] - 2026-07-15

### Fixed

- **Formatter**: Fixed a Django block that opens and closes on the same line (e.g. `{% component … %}{% endcomponent %}`) inside an inlined HTML element having its opening tag swallowed, leaving an unbalanced closing tag.

## [0.6.3] - 2026-07-08

### Fixed

- **Formatter**: Fixed parsing of a quoted attribute value containing a template tag with a nested quote of the same kind (e.g. `class="{% if x == "home" %}…"`), which previously mis-tokenized the tag and garbled its output.

## [0.6.2] - 2026-07-07

### Fixed

- **Formatter**: Fixed `<textarea>` closing when it contains wrapped content.
- **Formatter**: Fixed tokenizing tags with newlines or `>` inside quoted attribute values (e.g. a multi-line Alpine `x-data` containing arrow functions).
- **Formatter**: Fixed handling of malformed close tags (`</div` with no `>`), which previously swallowed following content.
- **Formatter**: Fixed closing-tag placement for empty elements not in `break_html_tags` (e.g. `<canvas>`, `<noscript>`).
- **Formatter**: Fixed `{% if %}` / `{% for %}` block expansion inside attribute values — for Alpine/JS object literals, SVG shape attributes, and values containing an HTML comment or `>` before the block — including keeping the block's closing marker on its own line.
- **Formatter**: Fixed an indent leak when `{# djlint:off #}` appears mid-tag, and fixed a self-contained `{# djlint:off #}`…`{# djlint:on #}` region inside a tag's attributes.
- **Formatter**: Matched djlint's quirk where a `djlint:off` region's `{% endif %}` suppresses line-breaking of later inline `{% endif %}` tags.
- **Formatter**: Fixed content staying inline after a tag with an unquoted `{{ … }}` attribute.
- **Formatter**: Preserved author-intended double spaces inside attribute values; only whitespace runs containing a newline are collapsed.
- **Formatter**: Fixed verbatim tags (`<pre>`, `<textarea>`) staying inline after preceding content, and stopped normalizing template-tag spacing (`{{x}}` → `{{ x }}`) inside verbatim tags (`<script>`, `<style>`, `<pre>`, `<textarea>`).
- **Formatter**: Preserved the closing marker on its own line for multi-line `{% … %}` tags (e.g. an `{% include %}` with each argument on its own line).
- **Formatter**: Fixed consecutive SVG shape elements (e.g. `<circle>`) staying on one line.
- **Formatter**: Fixed dedent of a multi-line text run whose last line ends with a closing inline tag.

### Changed

- **Formatter**: Consolidated `djlint:off` tag handling and template/break-tag classification into shared helpers.

## [0.6.1] - 2026-06-10

### Fixed

- **Formatter**: Fixed wrapping of unquoted Django attribute values.
- **Formatter**: Fixed tag collapse inside nested blocks.
- **Formatter**: Fixed indentation of unclosed `<td>` elements.
- **Formatter**: Fixed indentation of unclosed `<pre>` elements.
- **Formatter**: Fixed line breaks before closing tags in complex nested cases.
- **Formatter**: Fixed deletion of nested `<span>` elements.
- **Formatter**: Fixed indentation of inline tags and template variables.
- **Formatter**: Fixed indentation of `{% elif %}` blocks.
- **Formatter**: Fixed handling of conditionally unbalanced `<span>` elements.
- **Formatter**: Fixed `djlint:off` handling.
- **Formatter**: Fixed closing of multiline attribute lists.
- **Formatter**: Fixed closing tags for `<textarea>` and `<pre>`.
- **Formatter**: Fixed attribute wrapping for `<textarea>`.
- **Formatter**: Line lengths are now measured by character count (Unicode code points) rather than bytes, matching djlint's Python `len()`.

### Changed

- **Formatter**: Split the 2,400-line `formatter/mod.rs` into focused modules (`tag_format`, `tree`, `predicates`) while preserving byte-for-byte output parity. Centralized the per-line indent-tracking invariant into shared helpers and removed dead code.

### Performance

- **Formatter**: Cache `format_tag`'s attribute-matching regex in `OnceLock` statics instead of recompiling it for every tag, cutting the `format_large_template` benchmark from ~3.2s to ~0.12s (~26x).

## [0.6.0] - 2026-06-06

### Added

- **Config**: Added `use_gitignore` and `better_attribute_parsing` configuration options.

### Fixed

- **Formatter**: Fixed all non-whitespace tokens so they start on the correct source line.
- **Formatter**: Fixed collapsing of non-block elements containing only whitespace.
- **Formatter**: Fixed closing of nested verbatim tags.
- **Formatter**: Fixed leaked indent level after certain tag sequences.
- **Formatter**: Improved handling of Django block self-closing tags.
- **Formatter**: Fixed include expansion edge cases.
- **Formatter**: Fixed SVG formatting issues.
- **Formatter**: Fixed condensing logic incorrectly ignoring outer indentation.
- **Formatter**: Fixed indentation of multiline attributes followed by long text content.
- **Formatter**: Fixed indentation of long attribute lists.
- **Formatter**: Fixed attribute wrapping in combination with include expansion.
- **Formatter**: Preserved dot-namespaced custom element tag names (e.g. `my-lib.component`).
- **Formatter**: Fixed attribute wrapping for `<a>` tag content and tags with inline children.
- **Formatter**: Added `is_line_break` tag handling so line-break tags force a new line correctly.
- **Formatter**: Fixed ignored-block opening detection (`is_ignored_block_opening`).
- **Formatter**: Fixed indentation of self-closing tags.
- **Formatter**: Fixed collapse of `<textarea>` content.
- **Tags**: Added SVG structural tags (`g`, `defs`, `clipPath`, `mask`, `pattern`, `linearGradient`, `radialGradient`, `stop`, `text`, `tspan`) to the block tag set for correct formatting.

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
