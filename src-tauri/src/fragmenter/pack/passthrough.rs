use crate::fragmenter::segment::Atom;
use crate::fragmenter::output::FragmentMeta;
use super::RawFragment;

pub fn pack(atoms: Vec<Atom>) -> Vec<RawFragment> {
    atoms
        .into_iter()
        .map(|a| RawFragment {
            text: a.text.clone(),
            meta: FragmentMeta {
                source_range: a.range,
                heading_path: a.heading_path,
                suggested_title: a.heading_title,
                is_top_level_section: a.is_top_level,
                ..Default::default()
            },
        })
        .collect()
}
