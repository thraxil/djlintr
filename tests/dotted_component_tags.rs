//! Regression tests for dot-namespaced component tag names.
//!
//! Component frameworks such as django-cotton use a dot in the element
//! name to express namespacing, e.g. `<c-combobox.trigger>` is the
//! `trigger` sub-component of `c-combobox`. The tag-name grammar must keep
//! the whole name (including the `.`) intact; otherwise the closing tag is
//! rebuilt from a truncated name and the output becomes unbalanced
//! (`</c-combobox.trigger>` -> `</c-combobox>`), silently corrupting the
//! template.
//!
//! The plain-hyphen custom-element case (`<calendar-month>`) is covered by
//! the tag-name char class already; these tests pin down the dotted case.

use djlintr::config::Config;
use djlintr::format;

#[test]
fn dotted_component_closing_tag_is_preserved() {
    let config = Config::default();
    let src = "<c-combobox.trigger>{{ label }}</c-combobox.trigger>\n";
    let out = format(&config, src);
    assert!(
        out.contains("</c-combobox.trigger>"),
        "closing tag name must keep the `.trigger` suffix, got:\n{out}"
    );
}

#[test]
fn dotted_self_closing_component_tag_is_not_split() {
    let config = Config::default();
    let src = "<c-combobox.radio name=\"aspect\" value=\"a\" />\n";
    let out = format(&config, src);
    assert!(
        out.contains("<c-combobox.radio"),
        "opening tag name must not be split by a space, got:\n{out}"
    );
    assert!(
        !out.contains("<c "),
        "tag name must not be broken into `<c ...`, got:\n{out}"
    );
}

#[test]
fn nested_dotted_components_stay_balanced() {
    let config = Config::default();
    let src = "<c-combobox>\n  <c-combobox.trigger>{{ label }}</c-combobox.trigger>\n  <c-combobox.menu>\n    <c-combobox.radio name=\"aspect\" value=\"a\" />\n  </c-combobox.menu>\n</c-combobox>\n";
    let out = format(&config, src);
    for tag in [
        "</c-combobox.trigger>",
        "</c-combobox.menu>",
        "<c-combobox.radio",
    ] {
        assert!(out.contains(tag), "expected `{tag}` in output:\n{out}");
    }
}
