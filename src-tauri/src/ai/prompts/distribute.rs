pub fn build_batch_distribute_prompt(chunks: &[(&str, &str, &str)]) -> String {
    let mut chunks_input = String::new();
    for (i, (text, suggested_title, candidates_str)) in chunks.iter().enumerate() {
        chunks_input.push_str(&format!(
            "--- CHUNK #{} ---\n\
             Suggested Title: {}\n\
             Candidates:\n\
             {}\n\
             Content:\n\
             ```markdown\n\
             {}\n\
             ```\n\n",
             i, suggested_title, candidates_str, text
        ));
    }

    format!(
        "Analyze these markdown note chunks from a draft and recommend how to organize them into notes.\n\n\
         CHUNKS TO PROCESS:\n\
         {}\n\n\
         For each chunk, you must recommend one of the following actions:\n\
         1. \"append\": If the chunk belongs to or fits into one of its candidate notes. You MUST specify `target_note_id`.\n\
         2. \"create_child\": If the chunk should be created as a new note. You MUST specify `new_note_title`. Optionally, specify `parent_note_id` (if its parent is an existing note) OR `parent_chunk_index` (if its parent is another chunk in this plan, e.g. parent of chunk #2 is chunk #1, so `parent_chunk_index: 1`).\n\
         3. \"merge_with_chunk\": If this chunk should be merged/appended into another chunk in this same plan. You MUST specify `merge_target_chunk_index`.\n\
         4. \"skip\": If the chunk should be skipped or kept in inbox.\n\n\
         You MUST return a JSON array containing exactly one recommendation object for each chunk (matching the chunk indices in order). Each object MUST have the following format:\n\
         {{\n\
           \"action\": \"append\" | \"create_child\" | \"merge_with_chunk\" | \"skip\",\n\
           \"target_note_id\": null or number,\n\
           \"new_note_title\": null or string,\n\
           \"parent_note_id\": null or number,\n\
           \"parent_chunk_index\": null or number,\n\
           \"merge_target_chunk_index\": null or number,\n\
           \"tags\": null or array of strings (e.g. [\"#Chemistry/Microscope/Atom\", \"#important\"]),\n\
           \"reason\": \"a brief explanation for this recommendation\"\n\
         }}\n\n\
         Respond ONLY with a valid JSON array of objects. Do not include markdown code block formatting (e.g. do not wrap in ```json).",
         chunks_input
    )
}
