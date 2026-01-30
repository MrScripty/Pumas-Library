//! Search and filter models example

use pumas_library::{PumasApi, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Get path and query from args
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map(|s| s.as_str()).unwrap_or("./example-models");
    let query = args.get(2).map(|s| s.as_str()).unwrap_or("llama");

    println!("Initializing PumasApi with path: {}", path);

    let api = PumasApi::builder(path)
        .auto_create_dirs(true)
        .build()
        .await?;

    println!("Searching for '{}'...", query);
    let results = api.search_models(query, 10, 0).await?;

    println!("Found {} matches (showing first 10):", results.total_count);
    for model in results.models {
        println!("  - {} [{}] ({})", model.official_name, model.model_type, model.id);
    }

    Ok(())
}
