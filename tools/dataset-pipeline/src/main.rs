use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Prepare dataset features for drum classifier"
)]
struct Args {
    /// Path to an input JSON file with annotated hits
    input: PathBuf,
}

#[derive(Debug, Deserialize)]
struct AnnotationRecord {
    piece: String,
    velocity: u8,
    time: f64,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let file = File::open(&args.input)?;
    let reader = BufReader::new(file);
    let annotations: Vec<AnnotationRecord> = serde_json::from_reader(reader)?;
    info!(count = annotations.len(), "loaded annotations");
    println!("Loaded {} annotations", annotations.len());
    Ok(())
}
