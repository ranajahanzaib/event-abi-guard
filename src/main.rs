use clap::{Args, Parser, Subcommand, ValueEnum};
use eventabi_guard::{abi, consumer, report::Report};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

#[derive(Parser)]
#[command(
    name = "eventabi",
    version,
    about = "Protect declared off-chain consumers from event ABI changes"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Compare structural event changes without declaring consumer breakage.
    Diff {
        #[arg(long)]
        old: PathBuf,
        #[arg(long)]
        new: PathBuf,
        #[command(flatten)]
        output: Output,
    },
    /// Validate and check every declared consumer.
    Check {
        #[arg(long)]
        manifest: PathBuf,
        #[command(flatten)]
        output: Output,
    },
    /// Validate the manifest, input files and baseline expectations.
    Validate {
        #[arg(long)]
        manifest: PathBuf,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Human,
    Json,
    Markdown,
}
#[derive(Clone, Copy, ValueEnum)]
enum FailOn {
    Breaking,
    Warning,
    Never,
}
#[derive(Args)]
struct Output {
    #[arg(long, value_enum, default_value = "human")]
    format: Format,
    #[arg(long)]
    json_out: Option<PathBuf>,
    #[arg(long)]
    markdown_out: Option<PathBuf>,
    /// Emit escaped workflow commands to stderr, keeping JSON stdout clean.
    #[arg(long)]
    github_annotations: bool,
    #[arg(long, value_enum, default_value = "breaking")]
    fail_on: FailOn,
}
fn run(cli: Cli) -> Result<u8, Box<dyn std::error::Error>> {
    let (report, output) = match cli.command {
        Command::Validate { manifest } => {
            consumer::load(&manifest)?;
            println!("Manifest and baseline expectations are valid.");
            return Ok(0);
        }
        Command::Check { manifest, output } => {
            (consumer::check(&consumer::load(&manifest)?), output)
        }
        Command::Diff { old, new, output } => {
            let mut report = Report::new(1, 0);
            report.add_diff("contract", &abi::load(&old)?, &abi::load(&new)?);
            report.finish();
            (report, output)
        }
    };
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(path) = output.json_out {
        std::fs::write(path, format!("{json}\n"))?;
    }
    if let Some(path) = output.markdown_out {
        std::fs::write(path, report.markdown())?;
    }
    if output.github_annotations {
        io::stderr().lock().write_all(report.github().as_bytes())?;
    }
    let text = match output.format {
        Format::Human => report.human(),
        Format::Json => format!("{json}\n"),
        Format::Markdown => report.markdown(),
    };
    io::stdout().lock().write_all(text.as_bytes())?;
    let failed = match output.fail_on {
        FailOn::Breaking => report.summary.breaking > 0,
        FailOn::Warning => report.summary.breaking + report.summary.warnings > 0,
        FailOn::Never => false,
    };
    Ok(u8::from(failed))
}
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("eventabi: {e}");
            ExitCode::from(2)
        }
    }
}
