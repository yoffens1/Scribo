use scribo_lib::chunker::markdown::formatting::*;

#[test]
fn test_remove_empty_lines() {
    let input = "Line 1\n\n\nLine 2\n\nLine 3";
    assert_eq!(remove_empty_lines(input), "Line 1\nLine 2\nLine 3");
}

#[test]
fn test_remove_horizontal_rules() {
    assert_eq!(remove_horizontal_rules("Line 1\n---\nLine 2"), "Line 1\n\nLine 2");
    assert_eq!(remove_horizontal_rules("Line 1\n***\nLine 2"), "Line 1\n\nLine 2");
    assert_eq!(remove_horizontal_rules("Line 1\n___\nLine 2"), "Line 1\n\nLine 2");
}

#[test]
fn test_remove_links() {
    // Wiki links without alias
    assert_eq!(remove_links("Check [[Note Title]] here"), "Check Note Title here");
    // Wiki links with alias
    assert_eq!(remove_links("Check [[Note Title|Alias Name]] here"), "Check Alias Name here");
    // Markdown links
    assert_eq!(remove_links("Check [some text](http://example.com) here"), "Check some text here");
    // Mixed links
    assert_eq!(remove_links("[[A]] and [B](url)"), "A and B");
}

#[test]
fn test_remove_list_markers() {
    assert_eq!(remove_list_markers("- Item 1\n* Item 2\n+ Item 3"), "Item 1\nItem 2\nItem 3");
    assert_eq!(remove_list_markers("  - Indented"), "Indented");
}

#[test]
fn test_remove_list_numbering() {
    assert_eq!(remove_list_numbering("1. Item 1\n2. Item 2\n100. Item 3"), "Item 1\nItem 2\nItem 3");
    assert_eq!(remove_list_numbering("  1. Indented"), "Indented");
}

#[test]
fn test_remove_markdown_formatting() {
    assert_eq!(remove_markdown_formatting("**bold1** and __bold2__"), "bold1 and bold2");
    assert_eq!(remove_markdown_formatting("~~strike~~ and ==highlight=="), "strike and highlight");
    assert_eq!(remove_markdown_formatting("`code span`"), "code span");
    assert_eq!(remove_markdown_formatting("*italic1* and _italic2_"), "italic1 and italic2");
}

#[test]
fn test_strip_heading_markers() {
    assert_eq!(strip_heading_markers("# Heading 1\n## Heading 2\n###### Heading 6"), "Heading 1\nHeading 2\nHeading 6");
}
