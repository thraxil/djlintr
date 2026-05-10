# djlintr Plan - Porting djlint to Rust

The goal of this project is to port the Python tool `djlint` to Rust to improve performance and robustness. We will follow a TDD approach by porting test cases from the original repository first.

## Architecture Overview

- **Core Library**: Handles parsing, formatting, and linting logic.
- **CLI**: Handles file discovery, configuration, and user interaction.
- **Rules Engine**: A modular system for linting rules.
- **Template Engines**: Specialized handling for Django, Jinja2, Nunjucks, etc.

## Phase 1: Project Setup & Test Infrastructure

- [x] Refine `Cargo.toml` with necessary dependencies.
- [x] Create basic project structure.

## Phase 2: Test Porting (TDD Start)

- [x] Port `tests/test_html/test_basics.py` to `tests/formatter_basics.rs`.
- [x] Port `tests/test_linter/test_h005.py` to `tests/linter_h005.rs`.
- [ ] Implement a test harness that allows easy addition of new test cases from the original repo.

## Phase 3: Core Implementation (Formatter)

- [x] Implement basic HTML/Template tokenizer.
- [x] Implement indentation logic (basic).
- [ ] Implement attribute formatting (wrapping, sorting).
- [x] Verify against `formatter_basics.rs`.

## Phase 4: Core Implementation (Linter)

- [x] Implement the linter rule engine.
- [ ] Port all djlint rules:

### HTML Rules (H)
- [x] **H005**: Html tag should have lang attribute.
- [x] **H006**: Img tag should have height and width attributes.
- [x] **H007**: <!DOCTYPE ... > should be present before the html tag.
- [x] **H008**: Attributes should be double quoted.
- [x] **H009**: Tag names should be lowercase.
- [x] **H010**: Attribute names should be lowercase.
- [x] **H011**: Attribute values should be quoted.
- [x] **H012**: There should be no spaces around attribute =.
- [x] **H013**: Img tag should have an alt attribute.
- [x] **H014**: Found extra blank lines.
- [x] **H015**: Follow h tags with a line break.
- [x] **H016**: Missing title tag in html.
- [x] **H017**: Void tags should be self closing.
- [x] **H019**: Replace 'javascript:abc()' with on_ event and real url.
- [x] **H020**: Empty tag pair found. Consider removing.
- [x] **H021**: Inline styles should be avoided.
- [x] **H022**: Use HTTPS for external links.
- [x] **H023**: Do not use entity references.
- [x] **H024**: Omit type on scripts and styles.
- [x] **H025**: Tag seems to be an orphan. (Refining)
- [x] **H026**: Empty id and class tags can be removed.
- [x] **H029**: Consider using lowercase form method values.
- [x] **H030**: Consider adding a meta description.
- [x] **H031**: Consider adding meta keywords.
- [x] **H033**: Extra whitespace found in form action.
- [x] **H035**: Meta tags should be self closing.
- [x] **H036**: Avoid use of <br> tags.
- [x] **H037**: Duplicate attribute found.

### Template Rules (T)
- [x] **T001**: Variables should be wrapped in a whitespace.
- [x] **T002**: Double quotes should be used in tags.
- [x] **T003**: Endblock should have name. Ex: {% endblock body %}.
- [x] **T027**: Unclosed string found in template syntax.
- [x] **T028**: Consider using spaceless tags inside attribute values. {%- if/for -%}
- [x] **T032**: Extra whitespace found in template tags. (Implemented as part of T001)
- [x] **T034**: Did you intend to use {% ... %} instead of {% ... }%?

### Django Rules (D)
- [x] **D004**: (Django) Static urls should follow {% static path/to/file %} pattern.
- [x] **D018**: (Django) Internal links should use the {% url ... %} pattern.

### Jinja Rules (J)
- [x] **J004**: (Jinja) Static urls should follow {{ url_for('static'..) }} pattern.
- [x] **J018**: (Jinja) Internal links should use the {{ url_for() ... }} pattern.

## Phase 5: CLI & Performance

- [x] Implement CLI with `clap`.
- [x] Implement parallel file processing with `rayon`.
- [x] Add configuration support (`.djlintrc`, `pyproject.toml`).

## Phase 6: Refinement & Compatibility

- [ ] Support multiple template languages (Django, Jinja2, etc.).
- [x] Implement "Check" mode.
- [x] Benchmark and optimize.

## Phase 7: CI/CD & Deployment

- [x] Implement GitHub Actions workflow for cross-platform builds.
- [x] Automate test execution on PRs.
- [x] Automate binary releases for Linux, macOS, and Windows.

## Phase 8: Feature Parity (Configuration)

Implement missing configuration options from the original Python `djlint`:

- [ ] **Profile Support**: Support different template languages via `--profile` (`django`, `jinja`, `nunjucks`, `handlebars`, `golang`, `angular`, `html`).
- [ ] **Include Rules**: Add `--include` to specifically run only certain linter codes.
- [x] **Advanced Attribute Wrapping**: Implement `--max-attribute-length` (default: 70).
- [ ] **Gitignore Integration**: Implement `--use-gitignore` to honor `.gitignore` files.
- [ ] **Embedded Content Formatting**: 
    - [ ] `--format-css`: Format contents of `<style>` tags.
    - [ ] `--format-js`: Format contents of `<script>` tags.
    - [ ] `--indent-css` / `--indent-js`: Specific indentation for embedded content.
- [ ] **Preservation Options**:
    - [ ] `--preserve-blank-lines`: Keep existing blank lines.
    - [ ] `--preserve-leading-space`: Preserve leading space on text.
- [ ] **Tag Handling**:
    - [ ] `--close-void-tags`: Add closing marks to void tags (e.g., `<img />`).
    - [ ] `--ignore-case`: Do not fix the case of known HTML tags.
- [ ] **Block Handling**:
    - [ ] `--ignore-blocks`: List of template blocks to skip indenting.
    - [ ] `--blank-line-after-tag`: Add a blank line after specific tag groups.
- [ ] **Strictness & Scoping**:
    - [ ] `--require-pragma`: Only process files containing the `djlint:on` comment.
    - [ ] `per_file_ignores`: Map specific linter rules to specific files in config.
- [ ] **Attribute Template Tags**: `--format-attribute-template-tags` to format template syntax inside HTML attributes.
