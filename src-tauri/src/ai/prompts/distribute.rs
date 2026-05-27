pub fn build_distribute_prompt(chunk_text: &str, suggested_title: &str, candidates_str: &str) -> String {
    format!(
        "Analyze this markdown note chunk and recommend how to distribute/organize it.\n\n\
        CHUNK CONTENT:\n\
        ```markdown\n\
        {}\n\
        ```\n\n\
        SUGGESTED TITLE:\n\
        {}\n\n\
        CANDIDATE EXISTING NOTES:\n\
        {}\n\n\
        Choose one of these actions:\n\
        1. \"append\": If the chunk belongs/fits into one of the candidate notes. Provide `target_note_id`.\n\
        2. \"create_child\": If the chunk should be a new sub-note. Provide `new_note_title` and `parent_note_id` (optionally, the ID of a candidate note as its parent, or null if it should be at root level).\n\
        3. \"skip\": If the chunk should be skipped or kept in inbox.\n\n\
        You MUST return a JSON object with the following fields:\n\
        {{\n\
          \"action\": \"append\" | \"create_child\" | \"skip\",\n\
          \"target_note_id\": null or number,\n\
          \"new_note_title\": null or string,\n\
          \"parent_note_id\": null or number,\n\
          \"reason\": \"a brief explanation for this recommendation\"\n\
        }}\n\
        Respond ONLY with the JSON object. Do not include markdown code block syntax (like ```json).",
        chunk_text,
        suggested_title,
        candidates_str
    )
}
