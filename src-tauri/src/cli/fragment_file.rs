pub fn handle_fragment_file(file_path: &str, mode: &str) {
    let content = std::fs::read_to_string(file_path).expect("Could not read file");
    let default_opts = crate::fragmenter::FragmentConfig::default();
    
    println!("File: {}", file_path);
    
    match mode {
        "--embedding" => {
            let fragments = crate::fragmenter::fragment_for_embedding(&content, &default_opts);
            println!("Total Fragments (Embedding): {}", fragments.len());
            for (i, fragment) in fragments.iter().enumerate() {
                println!("\n================ FRAGMENT {} ================", i);
                println!("[Tokens: {}]", crate::fragmenter::token::count_tokens(fragment));
                println!("{}", fragment);
            }
        }
        "--generation" => {
            let fragments = crate::fragmenter::fragment_for_generation(&content, &default_opts);
            println!("Total Fragments (Generation): {}", fragments.len());
            for (i, fragment) in fragments.iter().enumerate() {
                println!("\n================ FRAGMENT {} ================", i);
                println!("[Tokens: {}]", crate::fragmenter::token::count_tokens(fragment));
                println!("{}", fragment);
            }
        }
        "--structural" => {
            let struct_opts = crate::fragmenter::FragmentConfig::structural();
            let result = crate::fragmenter::fragment_paired(content, &struct_opts);
            println!("Total Fragments (Structural): {}", result.pairs.len());
            for (i, pair) in result.pairs.iter().enumerate() {
                println!("\n================ FRAGMENT {} ================", i);
                println!("[Tokens: {}]", crate::fragmenter::token::count_tokens(&pair.embedding));
                println!("{}", pair.embedding);
            }
        }
        _ => { // --paired or default
            let result = crate::fragmenter::fragment_paired(content, &default_opts);
            println!("Total Fragments (Paired): {}", result.pairs.len());
            for (i, pair) in result.pairs.iter().enumerate() {
                println!("\n================ FRAGMENT {} ================", i);
                println!("[Tokens: {}]", crate::fragmenter::token::count_tokens(&pair.generation));
                println!("[Embedding]:\n{}\n", pair.embedding);
                println!("[Generation]:\n{}", pair.generation);
            }
        }
    }
}
