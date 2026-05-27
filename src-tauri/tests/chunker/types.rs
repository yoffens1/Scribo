use scribo_lib::fragmenter::{FragmentOptions, FragmentMode};

#[test]
fn test_default_options() {
    let opts = FragmentOptions::default();
    assert!(opts.lower_case);
    assert!(opts.remove_links);
    assert!(opts.remove_formatting);
    assert!(opts.format_latex);
    assert!(opts.remove_rules);
    assert!(opts.remove_numbering);
    assert!(opts.strip_heading_markers);
    assert!(opts.remove_list_markers);
    assert!(opts.compact_lines);
    assert!(opts.fragment_by_headings);
    assert_eq!(opts.heading_level, 2);
    assert!(opts.include_heading_in_fragments);
    assert!(!opts.separate_sub_headings);
    assert!(opts.keep_subheading_with_content);
    assert!(opts.preserve_tables);
    assert!(opts.linearize_tables);
    assert!(opts.each_table_row_as_separate_fragment);
    assert!(!opts.separate_tables_as_fragments);
    assert_eq!(opts.max_tokens, 256);
    assert_eq!(opts.overlap_tokens, 0);
}

#[test]
fn test_preset_embedding() {
    let base = FragmentOptions {
        max_tokens: 123,
        overlap_tokens: 45,
        preserve_tables: false,
        ..FragmentOptions::default()
    };
    let opts = base.for_mode(FragmentMode::Embedding);
    
    // Check invariants that must be preset
    assert!(opts.lower_case);
    assert!(opts.remove_links);
    assert!(opts.remove_formatting);
    assert!(opts.format_latex);
    assert!(opts.linearize_tables);
    assert!(opts.fragment_by_headings);
    assert_eq!(opts.heading_level, 2);
    assert!(opts.include_heading_in_fragments);
    assert!(opts.separate_sub_headings);
    assert!(opts.separate_tables_as_fragments);
    assert!(opts.keep_subheading_with_content);
    assert!(opts.remove_rules);
    assert!(opts.compact_lines);
    assert!(opts.remove_numbering);
    assert!(opts.strip_heading_markers);
    assert!(opts.remove_list_markers);
    assert!(opts.each_table_row_as_separate_fragment);
    
    // Non-preset fields must preserve user values
    assert_eq!(opts.max_tokens, 123);
    assert_eq!(opts.overlap_tokens, 45);
    assert!(!opts.preserve_tables);
}

#[test]
fn test_preset_generation() {
    let base = FragmentOptions {
        each_table_row_as_separate_fragment: false,
        ..FragmentOptions::default()
    };
    let opts = base.for_mode(FragmentMode::Generation);
    
    // Check invariants that must be preset
    assert!(opts.lower_case);
    assert!(opts.remove_links);
    assert!(opts.remove_formatting);
    assert!(!opts.format_latex);
    assert!(opts.remove_rules);
    assert!(opts.compact_lines);
    assert!(opts.remove_numbering);
    assert!(opts.strip_heading_markers);
    assert!(opts.remove_list_markers);
    assert!(opts.fragment_by_headings);
    assert_eq!(opts.heading_level, 2);
    assert!(!opts.include_heading_in_fragments);
    assert!(opts.separate_sub_headings);
    assert!(!opts.keep_subheading_with_content);
    assert!(!opts.linearize_tables);
    assert!(opts.separate_tables_as_fragments);
    assert!(opts.preserve_tables);
    assert_eq!(opts.max_tokens, usize::MAX);
    assert_eq!(opts.overlap_tokens, 0);
    
    // Non-preset fields must preserve user values
    assert!(!opts.each_table_row_as_separate_fragment);
}

#[test]
fn test_preset_structural() {
    let base = FragmentOptions {
        fragment_by_headings: false,
        heading_level: 4,
        include_heading_in_fragments: false,
        separate_sub_headings: true,
        keep_subheading_with_content: false,
        preserve_tables: false,
        separate_tables_as_fragments: true,
        linearize_tables: false,
        each_table_row_as_separate_fragment: false,
        max_tokens: 500,
        overlap_tokens: 50,
        ..FragmentOptions::default()
    };
    
    let opts = base.for_mode(FragmentMode::Structural);
    
    // Clean rules must be false
    assert!(!opts.lower_case);
    assert!(!opts.remove_links);
    assert!(!opts.remove_formatting);
    assert!(!opts.format_latex);
    assert!(!opts.remove_rules);
    assert!(!opts.remove_numbering);
    assert!(!opts.strip_heading_markers);
    assert!(!opts.remove_list_markers);
    assert!(!opts.compact_lines);
    
    // Custom structural attributes must remain
    assert!(!opts.fragment_by_headings);
    assert_eq!(opts.heading_level, 4);
    assert!(!opts.include_heading_in_fragments);
    assert!(opts.separate_sub_headings);
    assert!(!opts.keep_subheading_with_content);
    assert!(!opts.preserve_tables);
    assert!(opts.separate_tables_as_fragments);
    assert!(!opts.linearize_tables);
    assert!(!opts.each_table_row_as_separate_fragment);
    assert_eq!(opts.max_tokens, 500);
    assert_eq!(opts.overlap_tokens, 50);
}

/// Helper validator function that enforces option invariants dynamically.
/// Prints violations to stderr in ANSI red and panics.
pub fn validate_config_invariants(chunks: &[String], opts: &FragmentOptions) {
    for chunk in chunks {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 1. lower_case: true -> Check there are no uppercase characters.
        if opts.lower_case {
            if chunk.chars().any(|c| c.is_uppercase()) {
                report_failure("lower_case", chunk, "Found uppercase characters in output.");
            }
        }

        // 2. remove_links: true -> Check there are no Obsidian or markdown links.
        if opts.remove_links {
            if chunk.contains("[[") || chunk.contains("]]") {
                report_failure("remove_links", chunk, "Found wiki link brackets '[[' or ']]'.");
            }
            // Simple check for markdown links: contains "]("
            if chunk.contains("](") {
                report_failure("remove_links", chunk, "Found markdown link sequence ']('.");
            }
        }

        // 3. remove_rules: true -> Check there are no horizontal rules
        if opts.remove_rules {
            for line in chunk.lines() {
                let tl = line.trim();
                if tl == "---" || tl == "***" || tl == "___" {
                    report_failure("remove_rules", chunk, "Found horizontal rule line.");
                }
            }
        }

        // 4. remove_formatting: true -> Check for formatting symbols
        if opts.remove_formatting {
            // Since markdown stripping removes formatting wrappers, we check if standard double markers remain.
            if chunk.contains("**") || chunk.contains("~~") || chunk.contains("==") {
                report_failure("remove_formatting", chunk, "Found formatting markers (**, ~~ or ==).");
            }
        }

        // 5. strip_heading_markers: true -> No line starts with '#'
        if opts.strip_heading_markers {
            for line in chunk.lines() {
                if line.trim_start().starts_with('#') {
                    report_failure("strip_heading_markers", chunk, "Line starts with '#' heading marker.");
                }
            }
        }

        // 6. remove_list_markers: true -> No line starts with list markers
        if opts.remove_list_markers {
            for line in chunk.lines() {
                let tl = line.trim_start();
                if tl.starts_with("- ") || tl.starts_with("* ") || tl.starts_with("+ ") {
                    report_failure("remove_list_markers", chunk, "Line starts with list marker (- / * / +).");
                }
            }
        }

        // 7. remove_numbering: true -> No line starts with \d+.
        if opts.remove_numbering {
            for line in chunk.lines() {
                let tl = line.trim_start();
                if let Some(dot_idx) = tl.find(". ") {
                    let number_part = &tl[..dot_idx];
                    if !number_part.is_empty() && number_part.chars().all(|c| c.is_ascii_digit()) {
                        report_failure("remove_numbering", chunk, "Line starts with ordered list number.");
                    }
                }
            }
        }

        // 8. compact_lines: true -> Check for no consecutive empty lines or outer spacing
        if opts.compact_lines {
            if chunk.contains("\n\n\n") {
                report_failure("compact_lines", chunk, "Found three consecutive newlines.");
            }
            if chunk.starts_with('\n') || chunk.ends_with('\n') {
                report_failure("compact_lines", chunk, "Chunk has leading or trailing newline.");
            }
        }
    }
}

fn report_failure(rule: &str, chunk: &str, msg: &str) {
    eprintln!(
        "\x1b[31m[FAIL] Rule '{}' was violated!\nReason: {}\nChunk content:\n-------------------\n{}\n-------------------\x1b[0m",
        rule, msg, chunk
    );
    panic!("Invariant validation failed for rule: {}", rule);
}
