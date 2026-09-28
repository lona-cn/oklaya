mod telemetry;

mod server;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use indexmap::IndexMap;
use laya_inference::{
    Question, Request,
    model::{self, ModelKind},
    runtime::{Device, Laya},
};
use ort::ep::ExecutionProvider;
use std::{
    fs,
    io::{self, Read},
    net::SocketAddr,
    path::PathBuf,
    time::Instant,
};

#[derive(Parser)]
#[command(name = "laya", about = "Local Laya typed decision inference")]
struct Cli {
    #[arg(long, global = true, value_enum, default_value = "auto")]
    device: DeviceArg,
    #[arg(long, global = true, default_value = "multilingual")]
    model: String,
    #[arg(long, global = true)]
    max_len: Option<usize>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, Copy, ValueEnum)]
enum DeviceArg {
    Auto,
    Cpu,
    Cuda,
}
impl From<DeviceArg> for Device {
    fn from(value: DeviceArg) -> Self {
        match value {
            DeviceArg::Auto => Self::Auto,
            DeviceArg::Cpu => Self::Cpu,
            DeviceArg::Cuda => Self::Cuda,
        }
    }
}
#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long, default_value = "127.0.0.1:3000")]
        bind: SocketAddr,
    },
    Choice {
        #[arg(long)]
        text: String,
        #[arg(long)]
        question: String,
        #[arg(long = "option", required = true)]
        options: Vec<String>,
    },
    Bool {
        #[arg(long)]
        text: String,
        #[arg(long)]
        question: String,
    },
    Score {
        #[arg(long)]
        text: String,
        #[arg(long)]
        question: String,
        #[arg(long = "level", required = true)]
        levels: Vec<String>,
    },
    Predict {
        request: PathBuf,
    },
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    Inspect {
        path: PathBuf,
    },
    Bench {
        #[arg(long, default_value_t = 3)]
        repetitions: usize,
    },
}
#[derive(Subcommand)]
enum ModelCommand {
    Download { kind: String },
    List,
    Path { kind: String },
}

fn kind(name: &str) -> Result<ModelKind> {
    name.parse().map_err(|e| anyhow!("{e}"))
}
fn engine(cli: &Cli) -> Result<Laya> {
    let mut builder = Laya::builder()
        .model(kind(&cli.model)?)
        .device(cli.device.into());
    if let Some(len) = cli.max_len {
        builder = builder.max_len(len);
    }
    let engine = builder.build()?;
    eprintln!(
        "Model: {}\nProvider: {}\nModel path: {}",
        cli.model,
        engine.provider(),
        engine.model_path().display()
    );
    Ok(engine)
}
fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();
    let cli = Cli::parse();
    match &cli.command {
        Command::Serve { bind } => {
            let engine = engine(&cli)?;
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(server::serve(*bind, cli.model.clone(), engine))?;
        }
        Command::Model { command } => match command {
            ModelCommand::Download { kind: name } => {
                println!("{}", model::download(kind(name)?)?.display())
            }
            ModelCommand::Path { kind: name } => {
                println!("{}", model::path(kind(name)?)?.display())
            }
            ModelCommand::List => {
                for (name, present) in model::list()? {
                    println!("{name}\t{}", if present { "ready" } else { "missing" });
                }
            }
        },
        Command::Inspect { path } => {
            let session = ort::session::Session::builder()?.commit_from_file(path)?;
            let meta = session.metadata()?;
            println!(
                "Producer: {}; graph: {}",
                meta.producer().unwrap_or_default(),
                meta.name().unwrap_or_default()
            );
            println!("ONNX Runtime: {}", ort::info());
            println!("Session provider: CPU");
            println!(
                "Available providers in ORT build: CPU{}",
                if ort::ep::CUDA::default().is_available().unwrap_or(false) {
                    ", CUDA"
                } else {
                    ""
                }
            );
            for item in session.inputs() {
                println!("input\t{}\t{}", item.name(), item.dtype());
            }
            for item in session.outputs() {
                println!("output\t{}\t{}", item.name(), item.dtype());
            }
        }
        Command::Choice {
            text,
            question,
            options,
        } => {
            let criteria = options
                .iter()
                .map(|entry| {
                    let (label, description) = entry
                        .split_once('=')
                        .ok_or_else(|| anyhow!("--option requires label=description: {entry}"))?;
                    Ok((label.to_owned(), description.to_owned()))
                })
                .collect::<Result<IndexMap<_, _>>>()?;
            if criteria.len() != options.len() || criteria.contains_key("") {
                return Err(anyhow!("--option labels must be nonempty and unique"));
            }
            let questions = IndexMap::from([(
                "result".to_owned(),
                Question::Choice {
                    instructions: question.clone(),
                    criteria,
                },
            )]);
            let result = engine(&cli)?.predict(text, &questions)?;
            print_json(&result["result"])?;
        }
        Command::Bool { text, question } => {
            let questions = IndexMap::from([(
                "result".to_owned(),
                Question::Noul {
                    instructions: question.clone(),
                    criteria: Default::default(),
                },
            )]);
            let result = engine(&cli)?.predict(text, &questions)?;
            print_json(&result["result"])?;
        }
        Command::Score {
            text,
            question,
            levels,
        } => {
            let questions = IndexMap::from([(
                "result".to_owned(),
                Question::Score {
                    instructions: question.clone(),
                    criteria: levels.clone(),
                },
            )]);
            let result = engine(&cli)?.predict(text, &questions)?;
            print_json(&result["result"])?;
        }
        Command::Predict { request } => {
            let data = if request.to_string_lossy() == "-" {
                let mut buf = String::new();
                io::stdin().read_to_string(&mut buf)?;
                buf
            } else {
                fs::read_to_string(request)
                    .with_context(|| format!("read {}", request.display()))?
            };
            let req: Request = serde_json::from_str(&data).context("parse predict request")?;
            print_json(&engine(&cli)?.predict(&req.state, &req.questions)?)?;
        }
        Command::Bench { repetitions } => {
            if *repetitions == 0 {
                return Err(anyhow!("repetitions must be positive"));
            }
            let load = Instant::now();
            let mut engine = engine(&cli)?;
            let load_time = load.elapsed();
            let mut questions = IndexMap::new();
            for n in 0..50 {
                questions.insert(
                    format!("q{n}"),
                    Question::Noul {
                        instructions: format!("Is this issue urgent? {n}"),
                        criteria: Default::default(),
                    },
                );
            }
            let text = "Production database is unavailable. Users cannot access the service.";
            let first: IndexMap<_, _> = questions
                .iter()
                .take(1)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let warm = Instant::now();
            engine.predict(text, &first)?;
            let warm_time = warm.elapsed();
            eprintln!(
                "Model load: {:?}; warmup: {:?}; provider: {}",
                load_time,
                warm_time,
                engine.provider()
            );
            for count in [1, 5, 10, 50] {
                let subset: IndexMap<_, _> = questions
                    .iter()
                    .take(count)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if count != 1 {
                    engine.predict(text, &subset)?;
                }
                let mut times = Vec::with_capacity(*repetitions);
                for _ in 0..*repetitions {
                    let start = Instant::now();
                    engine.predict(text, &subset)?;
                    times.push(start.elapsed().as_secs_f64());
                }
                times.sort_by(f64::total_cmp);
                let p50 = times[(times.len() - 1) / 2];
                let p95 = times[((times.len() - 1) * 95).div_ceil(100)];
                println!(
                    "{count},{:.3},{:.3},{:.3}",
                    p50 * 1e3,
                    p95 * 1e3,
                    count as f64 / p50
                );
            }
        }
    }
    Ok(())
}
