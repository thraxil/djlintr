use djlintr::{config::Config, formatter::format};

// Block elements whose non-whitespace content spans multiple source lines
// should NOT be collapsed by djlint.  Tokens on the same source line CAN
// still collapse even when whitespace separators contain newlines.

#[test]
fn test_li_multiline_vars_not_collapsed() {
    // {{ a }} on line N, {{ b }} on line N+1 → keep expanded
    let input = "<ul>\n    <li>\n        {{ a }}:\n        {{ b }}\n    </li>\n</ul>\n";
    let expected = "<ul>\n    <li>\n        {{ a }}:\n        {{ b }}\n    </li>\n</ul>\n";
    assert_eq!(format(&Config::default(), input), expected);
}

#[test]
fn test_td_multiline_vars_not_collapsed() {
    let input = "<table>\n    <tr>\n        <td>\n            {{ a }}\n            {{ b }}\n        </td>\n    </tr>\n</table>\n";
    let expected = "<table>\n    <tr>\n        <td>\n            {{ a }}\n            {{ b }}\n        </td>\n    </tr>\n</table>\n";
    assert_eq!(format(&Config::default(), input), expected);
}

#[test]
fn test_p_single_line_content_collapses() {
    // Content is on one source line even though it's between newlines
    let input = "<p>\n    {{ x }} text\n</p>\n";
    let expected = "<p>{{ x }} text</p>\n";
    assert_eq!(format(&Config::default(), input), expected);
}
