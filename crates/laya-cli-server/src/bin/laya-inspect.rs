use anyhow::{Context, Result};
use clap::Parser;
use ort::ep::ExecutionProvider;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    model: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let session = ort::session::Session::builder()
        .context("initialize ONNX Runtime")?
        .commit_from_file(&args.model)
        .with_context(|| format!("load {}", args.model.display()))?;
    let meta = session.metadata()?;
    println!("Producer: {}", meta.producer().unwrap_or_default());
    println!("Graph: {}", meta.name().unwrap_or_default());
    println!("ONNX Runtime: {}", ort::info());
    let cuda = ort::ep::CUDA::default().is_available().unwrap_or(false);
    println!("Session provider: CPU");
    println!(
        "Available providers in ORT build: CPU{}",
        if cuda { ", CUDA" } else { "" }
    );
    println!("Inputs:");
    for outlet in session.inputs() {
        println!("  {}  {}", outlet.name(), outlet.dtype());
    }
    println!("Outputs:");
    for outlet in session.outputs() {
        println!("  {}  {}", outlet.name(), outlet.dtype());
    }
    Ok(())
}
