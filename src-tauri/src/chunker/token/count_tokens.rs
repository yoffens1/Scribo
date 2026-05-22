use std::sync::OnceLock;
use tiktoken_rs::{cl100k_base, CoreBPE};

static BPE: OnceLock<CoreBPE> = OnceLock::new();

pub fn get_bpe() -> &'static CoreBPE {
    BPE.get_or_init(|| cl100k_base().unwrap())
}

pub fn count_tokens(text: &str) -> usize {
    get_bpe().encode_with_special_tokens(text).len()
}
