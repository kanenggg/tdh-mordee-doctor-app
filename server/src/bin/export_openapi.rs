use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result};
use server::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() -> Result<()> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("server/openapi.json"));

    let document = ApiDoc::openapi()
        .to_pretty_json()
        .context("failed to serialize the OpenAPI document")?;

    fs::write(&output, document)
        .with_context(|| format!("failed to write OpenAPI document to {}", output.display()))?;

    println!("wrote OpenAPI document to {}", output.display());
    Ok(())
}
