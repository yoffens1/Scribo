pub mod token_budget;
pub mod char_budget;
pub mod passthrough;

use crate::fragmenter::output::FragmentMeta;

#[derive(Debug, Clone)]
pub struct RawFragment {
    pub text: String,
    pub meta: FragmentMeta,
}
