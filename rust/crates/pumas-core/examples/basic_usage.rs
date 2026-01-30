//! Basic usage example - list models in a directory

use pumas_library::{PumasApi, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Get path from args or use current directory
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./example-models".to_string());

    println!("Initializing PumasApi with path: {}", path);

    let api = PumasApi::builder(&path)
        .auto_create_dirs(true)
        .build()
        .await?;

    println!("Listing models...");
    let models = api.list_models().await?;

    if models.is_empty() {
        println!("No models found in library.");
    } else {
        println!("Found {} models:", models.len());
        for model in models {
            println!("  - {} ({})", model.official_name, model.id);
        }
    }

    Ok(())
}
