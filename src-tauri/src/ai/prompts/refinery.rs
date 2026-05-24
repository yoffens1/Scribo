use crate::ai::types::Message;

pub struct FragmentForTaxonomy<'a> {
    pub hash: &'a str,
    pub text: &'a str,
    pub source_path: &'a str,
}

pub fn build_atomize_prompt(fragment_text: &str, source_path: &str) -> Vec<Message> {
    vec![
        Message {
            role: "system".to_string(),
            content: r###"You are a knowledge curator. Given a note fragment, transform it into an atomic flashcard.
Your goal is to:
1. Formulate a SHORT question-style heading (like "## What is X?") that captures the single core concept in this fragment.
2. Determine an appropriate, clean Title Case filename in singular form (like "Virtual Private Network.md" or "Firewall.md") for the card. The filename must be a noun phrase, not a question.

If the fragment is a data table, use "## Table: [what this table shows]" and a matching filename like "Table of Elements.md".
If the fragment contains a cloze deletion (e.g., {{c1::...}}), formulate a suitable heading like "## Cloze: [concept]" and a matching filename like "Aufbau Principle Cloze.md".

You must respond with a JSON object of the following schema:
{
  "questionHeading": "## What is the Aufbau principle?",
  "filename": "Aufbau Principle.md"
}

Ensure the questionHeading starts with "## ".
Ensure the filename is in Title Case, ends with ".md", contains no invalid notesystem characters, and is a concise noun or concept name in singular form.

Do not include any Markdown wrapper, fences, or text outside the JSON object. Just return the raw JSON."###.to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!(
                "Source file: {}\n\nFragment:\n{}",
                source_path,
                fragment_text.chars().take(1500).collect::<String>()
            ),
        }
    ]
}

pub fn build_taxonomy_prompt(fragments: &[FragmentForTaxonomy], max_depth: u32) -> Vec<Message> {
    let mut fragment_list = String::new();
    for c in fragments {
        let snippet = c.text.chars().take(500).collect::<String>();
        fragment_list.push_str(&format!(
            "<fragment hash=\"{}\" source=\"{}\">\n{}\n</fragment>\n\n",
            c.hash, c.source_path, snippet
        ));
    }

    vec![
        Message {
            role: "system".to_string(),
            content: format!(
                r#"You are a knowledge librarian organizing unstructured notes into a clean folder hierarchy. 

Your task: given a set of note fragments, propose an ideal folder tree (max depth {}) that organizes them by topic.

Rules:
- Use clear, concise English folder names in Title Case (e.g. "Network Security", "Machine Learning"). Do NOT use hyphens for spaces.
- Group related fragments under shared parent folders.
- Do NOT create folders named after individual source notes — extract the topic.
- Each fragment should be placed in exactly one folder (no duplicates).
- Prefer breadth over depth — don't nest deeper than necessary.
- If a fragment could fit multiple folders, pick the most specific one.

Output format — valid JSON only, no markdown, no explanation:
{{
  "roots": [
    {{
      "name": "Folder Name",
      "description": "what this folder contains",
      "children": [...],
      "assignedFragments": ["hash1", "hash2"]
    }}
  ],
  "rationale": "brief explanation of your organization choices"
}}"#,
                max_depth
            ),
        },
        Message {
            role: "user".to_string(),
            content: format!("Here are the note fragments to organize:\n\n{}", fragment_list),
        }
    ]
}

pub fn build_placement_prompt(
    proposed_tree: &str,
    existing_tree: &str,
    fragments: &[FragmentForTaxonomy],
) -> Vec<Message> {
    let mut fragment_list = String::new();
    for c in fragments {
        let snippet = c.text.chars().take(500).collect::<String>();
        fragment_list.push_str(&format!(
            "<fragment hash=\"{}\">\n{}\n</fragment>\n\n",
            c.hash, snippet
        ));
    }

    vec![
        Message {
            role: "system".to_string(),
            content: r#"You are a knowledge architect resolving file placements into an existing folder tree.

You will be given:
1. An EXISTING folder tree.
2. A PROPOSED sub-tree of new folders/topics.
3. A list of note fragments with their hashes.

Your task is to map each fragment to a final output path (including filename if possible, otherwise just the folder path) by merging the PROPOSED structure into the EXISTING structure.
If the PROPOSED folder maps cleanly to an EXISTING folder, output the existing folder path.
If the PROPOSED folder represents a truly new topic, output a new folder path.

Rules:
1. Always output valid JSON.
2. Every fragment hash provided MUST have a placement decision.
3. Output paths should use forward slashes (e.g. "ExistingFolder/NewSubFolder").
4. Action should be "create" for new notes, "merge" to append to an existing file, "rename" if a file needs to be moved/renamed. If uncertain, just use "create".

Schema:
{
  "decisions": [
    {
      "fragmentHash": "...",
      "outputPath": "path/to/folder/or/file",
      "action": "create",
      "reason": "short explanation",
      "existingTarget": null
    }
  ],
  "foldersToCreate": ["path/to/new/folder"],
  "rationale": "overall explanation"
}"#.to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!(
                "Existing Tree:\n{}\n\nProposed Tree:\n{}\n\nFragments:\n{}",
                if existing_tree.is_empty() { "(empty)" } else { existing_tree },
                proposed_tree,
                fragment_list
            ),
        }
    ]
}
