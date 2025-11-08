use clap::Parser;
use taal_transcriber::{TranscriptionJob, TranscriptionPipeline};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(author, version, about = "Transcribe drum audio into notation", long_about = None)]
struct Cli {
    /// Path to the audio file to transcribe
    input: String,
    /// Title used for the generated lesson metadata
    #[arg(short, long, default_value = "Untitled Transcription")]
    title: String,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let pipeline = TranscriptionPipeline::new();
    let job = TranscriptionJob {
        audio_path: cli.input,
        title: cli.title,
    };
    let lesson = pipeline.transcribe(&job)?;
    let exporter = taal_domain::io::JsonExporter;
    let bytes = exporter.export(&lesson, taal_domain::ExportFormat::Json)?;
    println!("{}", String::from_utf8_lossy(&bytes));
    Ok(())
}
