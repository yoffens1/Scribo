//! General language detection utilities.
//!
//! Wraps the `whatlang` crate and maps its internal [`Lang`] enum to compact
//! ISO-639-1 two-letter codes. Unknown or unsupported languages default to `"en"`.

use whatlang::{detect, Lang};

/// Maps a `whatlang` language tag to the corresponding ISO-639-1 two-letter code.
/// Falls back to `"en"` for any language not explicitly listed.
pub fn lang_to_iso639_1(lang: Lang) -> &'static str {
    match lang {
        Lang::Eng => "en",
        Lang::Rus => "ru",
        Lang::Ukr => "uk",
        Lang::Bel => "be",
        Lang::Deu => "de",
        Lang::Fra => "fr",
        Lang::Spa => "es",
        Lang::Ita => "it",
        Lang::Cmn => "zh",
        Lang::Jpn => "ja",
        Lang::Por => "pt",
        Lang::Pol => "pl",
        Lang::Nld => "nl",
        _ => "en",
    }
}

/// Detects the language of `text` and returns its ISO-639-1 code.
/// Returns `None` when `whatlang` cannot determine the language
/// (e.g. the input is too short or consists entirely of punctuation/numbers).
pub fn detect_language(text: &str) -> Option<String> {
    let info = detect(text);
    info.map(|i| lang_to_iso639_1(i.lang()).to_string())
}

/// Returns `true` iff the detected language is English.
/// Returns `false` for ambiguous/undetectable input.
pub fn is_english(text: &str) -> bool {
    if let Some(info) = detect(text) {
        info.lang() == Lang::Eng
    } else {
        false
    }
}

/// Picks the dominant language from a set of texts.
/// Uses MIN_TEXT_LEN_FOR_LANG (from constants) to skip too-short snippets.
pub fn pick_dominant_language(texts: &[String]) -> String {
    use std::collections::HashMap;
    if texts.is_empty() {
        return "en".to_string();
    }

    let mut counts = HashMap::new();
    for text in texts {
        if text.trim().len() < crate::core::constants::MIN_TEXT_LEN_FOR_LANG {
            continue;
        }
        if let Some(lang) = detect_language(text) {
            *counts.entry(lang).or_insert(0) += 1;
        }
    }

    let mut best_lang = "en".to_string();
    let mut max_count = 0;
    for (l, c) in counts {
        if c > max_count {
            max_count = c;
            best_lang = l;
        }
    }
    best_lang
}
