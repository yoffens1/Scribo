use whatlang::{detect, Lang};

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

pub fn detect_language(text: &str) -> Option<String> {
    let info = detect(text);
    info.map(|i| lang_to_iso639_1(i.lang()).to_string())
}

pub fn is_english(text: &str) -> bool {
    if let Some(info) = detect(text) {
        info.lang() == Lang::Eng
    } else {
        false
    }
}
