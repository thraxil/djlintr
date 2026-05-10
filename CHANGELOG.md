# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
