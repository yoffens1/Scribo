use scribo_lib::fragmenter::{FragmentConfig, CleanFlags, Segmenter, Packer, LatexHandling};

#[test]
fn test_default_config() {
    let config = FragmentConfig::default();
    let flags = config.cleaner.to_flags();
    assert!(flags.lower_case);
    assert!(flags.remove_links);
    assert!(flags.remove_formatting);
    assert_eq!(flags.latex, LatexHandling::Format);
    assert!(flags.remove_rules);
    assert!(flags.remove_numbering);
    assert!(flags.strip_heading_markers);
    assert!(flags.remove_list_markers);
    assert!(flags.compact_lines);
    
    match config.segmenter {
        Segmenter::HeadingSections { level, keep_subheading_with_content, separate_sub_headings } => {
            assert_eq!(level, 2);
            assert!(keep_subheading_with_content);
            assert!(separate_sub_headings);
        }
        _ => panic!("Expected HeadingSections segmenter"),
    }
    
    assert!(config.include_heading_in_fragments);
    assert!(flags.preserve_tables);
    assert!(flags.linearize_tables);
    assert!(flags.each_table_row_as_separate_fragment);
    assert!(flags.separate_tables_as_fragments);
    
    match config.packer {
        Packer::TokenBudget { max_tokens, overlap_tokens } => {
            assert_eq!(max_tokens, 256);
            assert_eq!(overlap_tokens, 0);
        }
        _ => panic!("Expected TokenBudget packer"),
    }
}

#[test]
fn test_preset_embedding() {
    let config = FragmentConfig::embedding();
    let flags = config.cleaner.to_flags();
    
    assert!(flags.lower_case);
    assert!(flags.remove_links);
    assert!(flags.remove_formatting);
    assert_eq!(flags.latex, LatexHandling::Format);
    assert!(flags.linearize_tables);
    assert!(flags.separate_tables_as_fragments);
    assert!(flags.remove_rules);
    assert!(flags.compact_lines);
    assert!(flags.remove_numbering);
    assert!(flags.strip_heading_markers);
    assert!(flags.remove_list_markers);
    assert!(flags.each_table_row_as_separate_fragment);
    
    match config.segmenter {
        Segmenter::HeadingSections { level, keep_subheading_with_content, separate_sub_headings } => {
            assert_eq!(level, 2);
            assert!(keep_subheading_with_content);
            assert!(separate_sub_headings);
        }
        _ => panic!("Expected HeadingSections segmenter"),
     }
}

#[test]
fn test_preset_generation() {
    let config = FragmentConfig::generation();
    let flags = config.cleaner.to_flags();
    
    assert!(flags.lower_case);
    assert!(flags.remove_links);
    assert!(flags.remove_formatting);
    assert_eq!(flags.latex, LatexHandling::Keep);
    assert!(flags.remove_rules);
    assert!(flags.compact_lines);
    assert!(flags.remove_numbering);
    assert!(flags.strip_heading_markers);
    assert!(flags.remove_list_markers);
    
    match config.segmenter {
        Segmenter::HeadingSections { level, keep_subheading_with_content, separate_sub_headings } => {
            assert_eq!(level, 2);
            assert!(!keep_subheading_with_content);
            assert!(separate_sub_headings);
        }
        _ => panic!("Expected HeadingSections segmenter"),
    }
    
    assert!(!config.include_heading_in_fragments);
    assert!(!flags.linearize_tables);
    assert!(flags.separate_tables_as_fragments);
    assert!(flags.preserve_tables);
    
    match config.packer {
        Packer::Passthrough => {}
        _ => panic!("Expected Passthrough packer"),
    }
}

#[test]
fn test_preset_structural() {
    let config = FragmentConfig::structural();
    let flags = config.cleaner.to_flags();
    
    assert!(!flags.lower_case);
    assert!(!flags.remove_links);
    assert!(!flags.remove_formatting);
    assert_eq!(flags.latex, LatexHandling::Keep);
    assert!(!flags.remove_rules);
    assert!(!flags.remove_numbering);
    assert!(!flags.strip_heading_markers);
    assert!(!flags.remove_list_markers);
    assert!(!flags.compact_lines);
}

pub fn validate_config_invariants(chunks: &[String], opts: &CleanFlags) {
    for chunk in chunks {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }

        if opts.lower_case {
            if chunk.chars().any(|c| c.is_uppercase()) {
                report_failure("lower_case", chunk, "Found uppercase characters in output.");
            }
        }

        if opts.remove_links {
            if chunk.contains("[[") || chunk.contains("]]") {
                report_failure("remove_links", chunk, "Found wiki link brackets '[[' or ']]'.");
            }
            if chunk.contains("](") {
                report_failure("remove_links", chunk, "Found markdown link sequence ']('.");
            }
        }

        if opts.remove_rules {
            for line in chunk.lines() {
                let tl = line.trim();
                if tl == "---" || tl == "***" || tl == "___" {
                    report_failure("remove_rules", chunk, "Found horizontal rule line.");
                }
            }
        }

        if opts.remove_formatting {
            if chunk.contains("**") || chunk.contains("~~") || chunk.contains("==") {
                report_failure("remove_formatting", chunk, "Found formatting markers (**, ~~ or ==).");
            }
        }

        if opts.strip_heading_markers {
            for line in chunk.lines() {
                if line.trim_start().starts_with('#') {
                    report_failure("strip_heading_markers", chunk, "Line starts with '#' heading marker.");
                }
            }
        }

        if opts.remove_list_markers {
            for line in chunk.lines() {
                let tl = line.trim_start();
                if tl.starts_with("- ") || tl.starts_with("* ") || tl.starts_with("+ ") {
                    report_failure("remove_list_markers", chunk, "Line starts with list marker (- / * / +).");
                }
            }
        }

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
