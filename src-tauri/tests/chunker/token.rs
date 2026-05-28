use scribo_lib::fragmenter::token::*;

#[test]
fn test_count_tokens() {
    let text = "Hello world from scribo chunker.";
    let tokens = count_tokens(text);
    // 5 words * 1.5 = 7.5. Max of 7.5 and chars (32) / 3.0 (10.66) -> 10.66. Ceil is 11.
    assert_eq!(tokens, 11);
}

#[test]
fn test_split_by_sentence_boundaries() {
    let text = "Sentence one. Sentence two! Sentence three?";
    
    // Set max_tokens high enough to keep all, or split
    let chunks = split_by_sentence_boundaries(text, 10);
    assert!(chunks.len() >= 2);
    assert_eq!(chunks[0].0, "Sentence one.");
}

#[test]
fn test_split_by_words() {
    let text = "one two three four five";
    // Each word has some tokens, limit to small count to force splitting
    let chunks = split_by_words(text, 3);
    assert!(chunks.len() > 1);
    
    // Reassembled text must equal original minus spacing
    let reassembled: Vec<String> = chunks.into_iter().map(|(s, _)| s).collect();
    let reassembled_str = reassembled.join(" ");
    assert_eq!(reassembled_str, text);
}

#[test]
fn test_split_oversized_paragraph() {
    let text = "Line one.\nLine two.\nLine three.";
    let chunks = split_oversized_paragraph(text, 4);
    assert!(chunks.len() > 1);
}
