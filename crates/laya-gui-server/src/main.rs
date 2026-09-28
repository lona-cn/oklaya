use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State, rejection::JsonRejection},
    http::{HeaderMap, Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{process::Child, sync::Mutex};

#[derive(Parser)]
#[command(about = "Local graphical administration for the Laya inference service")]
struct Args {
    /// Private address for the administration dashboard.
    #[arg(long, default_value = "127.0.0.1:8081")]
    bind: SocketAddr,
    /// Address reserved for the locally managed inference service.
    #[arg(long, default_value = "127.0.0.1:3000")]
    api_bind: SocketAddr,
    /// Path to the laya inference executable (default: sibling of this executable).
    #[arg(long)]
    laya_bin: Option<PathBuf>,
    /// Serve the browser dashboard without opening a desktop window.
    #[arg(long)]
    no_window: bool,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Model {
    Multilingual,
    English,
    TypedDecisions,
}
impl Model {
    fn as_str(self) -> &'static str {
        match self {
            Self::Multilingual => "multilingual",
            Self::English => "english",
            Self::TypedDecisions => "typed-decisions",
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Device {
    Auto,
    Cpu,
    Cuda,
}
impl Device {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartRequest {
    model: Model,
    device: Device,
}

struct Launch {
    child: Child,
    model: Model,
    device: Device,
    since: Instant,
}

#[derive(Default)]
struct Managed {
    launch: Option<Launch>,
    last_failure: Option<String>,
}

#[derive(Clone)]
struct AppState {
    managed: Arc<Mutex<Managed>>,
    http: reqwest::Client,
    bin: PathBuf,
    api_bind: SocketAddr,
    host: String,
}

#[derive(Deserialize, Serialize)]
struct Health {
    status: String,
    model: String,
    provider: String,
}

#[derive(Deserialize, Serialize)]
struct Stats {
    total_requests: u64,
    succeeded: u64,
    failed: u64,
    average_latency_ms: f64,
}

#[derive(Deserialize, Serialize)]
struct RequestSummary {
    id: u64,
    at_unix_ms: u64,
    status: String,
    duration_ms: u64,
    question_count: usize,
}

#[derive(Deserialize, Serialize)]
struct History {
    items: Vec<RequestSummary>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct Status {
    state: &'static str,
    model: Option<Model>,
    device: Option<Device>,
    pid: Option<u32>,
    uptime_seconds: Option<u64>,
    health: Option<Health>,
    message: Option<String>,
}

fn error(status: StatusCode, code: &'static str, message: impl AsRef<str>) -> Response {
    (
        status,
        Json(json!({"error": {"code": code, "message": message.as_ref()}})),
    )
        .into_response()
}

fn refresh(managed: &mut Managed) {
    let Some(launch) = managed.launch.as_mut() else {
        return;
    };
    match launch.child.try_wait() {
        Ok(Some(exit)) => {
            managed.last_failure = Some(format!(
                "Inference service exited ({}). Check the selected model, device and CLI installation.",
                exit
            ));
            managed.launch = None;
        }
        Ok(None) => {}
        Err(_) => {
            managed.last_failure = Some("Could not inspect the inference service process.".into());
            managed.launch = None;
        }
    }
}

async fn status(State(state): State<AppState>) -> Json<Status> {
    let (model, device, pid, since, failure) = {
        let mut managed = state.managed.lock().await;
        refresh(&mut managed);
        match managed.launch.as_ref() {
            Some(launch) => (
                Some(launch.model),
                Some(launch.device),
                launch.child.id(),
                Some(launch.since.elapsed().as_secs()),
                None,
            ),
            None => (None, None, None, None, managed.last_failure.clone()),
        }
    };
    let health = if model.is_some() {
        state
            .http
            .get(format!("http://{}/api/v1/health", state.api_bind))
            .send()
            .await
            .ok()
            .and_then(|r| r.error_for_status().ok())
    } else {
        None
    };
    let health = match health {
        Some(response) => response.json::<Health>().await.ok().filter(|value| {
            value.status == "ok" && Some(value.model.as_str()) == model.map(Model::as_str)
        }),
        None => None,
    };
    let ready = health.is_some();
    Json(Status {
        state: if model.is_none() {
            if failure.is_some() {
                "failed"
            } else {
                "stopped"
            }
        } else if ready {
            "running"
        } else {
            "starting"
        },
        model,
        device,
        pid,
        uptime_seconds: since,
        health,
        message: if model.is_some() && !ready {
            Some("Preparing model and initializing inference (the CLI downloads missing models automatically)…".into())
        } else {
            failure
        },
    })
}

async fn start(
    State(state): State<AppState>,
    payload: Result<Json<StartRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match payload {
        Ok(value) => value,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Expected JSON with a valid model and device.",
            );
        }
    };
    let mut managed = state.managed.lock().await;
    refresh(&mut managed);
    if managed.launch.is_some() {
        return error(
            StatusCode::CONFLICT,
            "already_running",
            "Stop the owned inference service before starting another.",
        );
    }
    // Refuse to start over an unrelated server. The child also reports a bind conflict if
    // another process wins the short gap between this check and its own bind.
    if tokio::net::TcpListener::bind(state.api_bind).await.is_err() {
        return error(
            StatusCode::CONFLICT,
            "port_in_use",
            "The inference API address is already in use; no process was started.",
        );
    }
    let bin = match std::fs::canonicalize(&state.bin) {
        Ok(path) if path.is_file() => path,
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "missing_binary",
                "The laya executable was not found at the configured path.",
            );
        }
    };
    let child = tokio::process::Command::new(bin)
        .args([
            "--model",
            request.model.as_str(),
            "--device",
            request.device.as_str(),
            "serve",
            "--bind",
        ])
        .arg(state.api_bind.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn();
    match child {
        Ok(child) => {
            managed.launch = Some(Launch {
                child,
                model: request.model,
                device: request.device,
                since: Instant::now(),
            });
            managed.last_failure = None;
            (StatusCode::ACCEPTED, Json(json!({"state": "starting"}))).into_response()
        }
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "spawn_failed",
            "Could not launch the configured laya executable.",
        ),
    }
}

async fn stop(State(state): State<AppState>) -> Response {
    let mut managed = state.managed.lock().await;
    refresh(&mut managed);
    let Some(mut launch) = managed.launch.take() else {
        return error(
            StatusCode::CONFLICT,
            "not_running",
            "There is no owned inference service to stop.",
        );
    };
    if launch.child.kill().await.is_err() {
        managed.launch = Some(launch);
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stop_failed",
            "Could not stop the owned inference service.",
        );
    }
    let _ = launch.child.wait().await;
    managed.last_failure = None;
    Json(json!({"state": "stopped"})).into_response()
}

async fn telemetry(State(state): State<AppState>) -> Response {
    let model = {
        let mut managed = state.managed.lock().await;
        refresh(&mut managed);
        let Some(launch) = managed.launch.as_ref() else {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "not_running",
                "No owned inference service is running.",
            );
        };
        launch.model
    };
    let base = format!("http://{}", state.api_bind);
    let healthy = match state.http.get(format!("{base}/api/v1/health")).send().await {
        Ok(response) => match response.error_for_status() {
            Ok(response) => response
                .json::<Health>()
                .await
                .ok()
                .is_some_and(|health| health.status == "ok" && health.model == model.as_str()),
            Err(_) => false,
        },
        Err(_) => false,
    };
    if !healthy {
        return error(
            StatusCode::BAD_GATEWAY,
            "api_unavailable",
            "The owned inference API has not become available yet.",
        );
    }
    let (stats, history) = tokio::join!(
        state.http.get(format!("{base}/api/v1/stats")).send(),
        state
            .http
            .get(format!("{base}/api/v1/requests?limit=50"))
            .send(),
    );
    let (Ok(stats), Ok(history)) = (stats, history) else {
        return error(
            StatusCode::BAD_GATEWAY,
            "api_unavailable",
            "The inference API has not become available yet.",
        );
    };
    let (Ok(stats), Ok(history)) = (stats.error_for_status(), history.error_for_status()) else {
        return error(
            StatusCode::BAD_GATEWAY,
            "api_unavailable",
            "The inference API telemetry is unavailable.",
        );
    };
    match tokio::try_join!(stats.json::<Stats>(), history.json::<History>()) {
        Ok((stats, history)) => Json(json!({"stats": stats, "history": history})).into_response(),
        Err(_) => error(
            StatusCode::BAD_GATEWAY,
            "api_unavailable",
            "The inference API returned invalid telemetry.",
        ),
    }
}

async fn check_local_origin(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let headers: &HeaderMap = request.headers();
    if headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        != Some(state.host.as_str())
    {
        return error(
            StatusCode::FORBIDDEN,
            "invalid_host",
            "This dashboard is available only through its local address.",
        );
    }
    if request.method() == Method::POST
        && headers.get(header::ORIGIN).is_some_and(|origin| {
            origin.to_str().ok() != Some(format!("http://{}", state.host).as_str())
        })
    {
        return error(
            StatusCode::FORBIDDEN,
            "invalid_origin",
            "Cross-origin administration is not allowed.",
        );
    }
    next.run(request).await
}

async fn index() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_str!("../assets/index.html"),
    )
}
async fn css() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_str!("../assets/app.css"),
    )
}
async fn js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_str!("../assets/app.js"),
    )
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.css", get(css))
        .route("/app.js", get(js))
        .route("/admin/status", get(status))
        .route("/admin/telemetry", get(telemetry))
        .route("/admin/start", post(start))
        .route("/admin/stop", post(stop))
        .layer(DefaultBodyLimit::max(4096))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            check_local_origin,
        ))
        .with_state(state)
}

async fn shutdown(state: &AppState) {
    let mut managed = state.managed.lock().await;
    if let Some(mut launch) = managed.launch.take() {
        let _ = launch.child.kill().await;
        let _ = launch.child.wait().await;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !args.bind.ip().is_loopback()
        || !args.api_bind.ip().is_loopback()
        || args.api_bind.port() == 0
    {
        return Err(
            "--bind and --api-bind must be loopback addresses; --api-bind needs a fixed port"
                .into(),
        );
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let listener = runtime.block_on(tokio::net::TcpListener::bind(args.bind))?;
    let bound = listener.local_addr()?;
    let bin = match args.laya_bin {
        Some(path) => path,
        None => {
            std::env::current_exe()?.with_file_name(if cfg!(windows) { "laya.exe" } else { "laya" })
        }
    };
    let state = AppState {
        managed: Arc::new(Mutex::new(Managed::default())),
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()?,
        bin,
        api_bind: args.api_bind,
        host: bound.to_string(),
    };
    println!("Laya admin listening on http://{bound}");
    if args.no_window {
        let result = runtime.block_on(async {
            axum::serve(listener, router(state.clone()))
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await
        });
        runtime.block_on(shutdown(&state));
        result?;
    } else {
        let server_state = state.clone();
        let server =
            runtime.spawn(async move { axum::serve(listener, router(server_state)).await });
        let url: tauri::Url = format!("http://{bound}").parse()?;
        let result = tauri::Builder::default()
            .setup(move |app| {
                tauri::WebviewWindowBuilder::new(app, "admin", tauri::WebviewUrl::External(url))
                    .title("Laya · Operations console")
                    .inner_size(1200.0, 820.0)
                    .min_inner_size(820.0, 620.0)
                    .build()?;
                Ok(())
            })
            .run(tauri::generate_context!());
        server.abort();
        runtime.block_on(shutdown(&state));
        result?;
    }
    Ok(())
}
