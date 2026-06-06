use djlintr::{config::Config, formatter::format};

// djlint only collapses empty elements whose tag appears in
// optional_single_line_html_tags (is_condensable_tag).  SVG shape tags like
// <circle> are not in that list, so <circle>\n</circle> stays expanded.
// <path> IS in the list, so it collapses even when the close tag is on the
// next source line.

#[test]
fn test_circle_empty_stays_expanded() {
    let input = "<circle cx=\"12\">\n</circle>\n";
    let expected = "<circle cx=\"12\">\n</circle>\n";
    assert_eq!(format(&Config::default(), input), expected);
}

#[test]
fn test_path_empty_collapses() {
    let input = "<path d=\"M19 9l-7 7-7-7\">\n</path>\n";
    let expected = "<path d=\"M19 9l-7 7-7-7\"></path>\n";
    assert_eq!(format(&Config::default(), input), expected);
}
