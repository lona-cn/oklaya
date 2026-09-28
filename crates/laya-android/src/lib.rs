mod lan;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    path::PathBuf,
    sync::{
        Arc, Mutex as EngineMutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use indexmap::IndexMap;
use laya_inference::{
    DecisionResult, Request,
    model::{self, ModelKind},
    runtime::{Device, Laya},
};
use parking_lot::Mutex as StateMutex;
use serde::Serialize;
use tauri::Manager;
use tokio::sync::oneshot;

#[derive(Serialize)]
struct MobileStatus {
    model_ready: bool,
    provider: Option<String>,
    lan_port: Option<u16>,
    lan_token: Option<String>,
    lan_url: Option<String>,
}

struct LanService {
    port: u16,
    url: String,
    token: String,
    stop: oneshot::Sender<()>,
    task: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Default)]
struct NativeState {
    engine: Option<Arc<EngineMutex<Laya>>>,
    provider: Option<String>,
    service: Option<LanService>,
}

struct MobileApp {
    data_dir: PathBuf,
    state: StateMutex<NativeState>,
    operation: tokio::sync::Mutex<()>,
    foreground: AtomicBool,
}

impl MobileApp {
    fn status(&self) -> MobileStatus {
        let mut state = self.state.lock();
        if state
            .service
            .as_ref()
            .is_some_and(|service| service.task.inner().is_finished())
        {
            state.service.take();
        }
        MobileStatus {
            model_ready: state.engine.is_some(),
            provider: state.provider.clone(),
            lan_port: state.service.as_ref().map(|service| service.port),
            lan_token: state.service.as_ref().map(|service| service.token.clone()),
            lan_url: state.service.as_ref().map(|service| service.url.clone()),
        }
    }

    fn stop_immediately(&self) {
        let mut state = self.state.lock();
        if let Some(service) = state.service.take() {
            let _ = service.stop.send(());
            service.task.abort();
        }
    }

    fn model_dir(&self) -> PathBuf {
        self.data_dir
            .join("models")
            .join(model::REVISION)
            .join(ModelKind::Multilingual.as_str())
    }
}

#[tauri::command]
fn mobile_status(app: tauri::State<'_, MobileApp>) -> MobileStatus {
    app.status()
}

#[tauri::command]
async fn mobile_download_model(app: tauri::State<'_, MobileApp>) -> Result<MobileStatus, String> {
    let _operation = app.operation.lock().await;
    let directory = app.model_dir();
    tokio::task::spawn_blocking(move || model::download_to(ModelKind::Multilingual, &directory))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(app.status())
}

#[tauri::command]
async fn mobile_load_model(app: tauri::State<'_, MobileApp>) -> Result<MobileStatus, String> {
    let _operation = app.operation.lock().await;
    if app.state.lock().engine.is_some() {
        return Ok(app.status());
    }
    let directory = app.model_dir();
    let engine = tokio::task::spawn_blocking(move || {
        // Verify all pinned artifacts, including the graph, before passing the local
        // model directory to the builder (which otherwise checks only existence).
        model::path_in(ModelKind::Multilingual, &directory)?;
        Laya::builder()
            .model(ModelKind::Multilingual)
            .model_path(directory)
            .device(Device::Cpu)
            .build()
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    let provider = engine.provider().to_owned();
    let mut state = app.state.lock();
    state.engine = Some(Arc::new(EngineMutex::new(engine)));
    state.provider = Some(provider);
    drop(state);
    Ok(app.status())
}

#[tauri::command]
async fn mobile_predict(
    app: tauri::State<'_, MobileApp>,
    request: Request,
) -> Result<IndexMap<String, DecisionResult>, String> {
    let engine = app
        .state
        .lock()
        .engine
        .clone()
        .ok_or("Load the model first")?;
    tokio::task::spawn_blocking(move || {
        engine
            .lock()
            .map_err(|_| "Model engine unavailable".to_string())?
            .predict(&request.state, &request.questions)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn lan_address() -> Result<Ipv4Addr, String> {
    let usable = |ip: Ipv4Addr| !ip.is_loopback() && !ip.is_unspecified() && !ip.is_multicast();
    // The default outbound interface is usually the Wi-Fi interface the user
    // expects; probing the route does not transmit a datagram.
    if let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        if socket.connect((Ipv4Addr::new(1, 1, 1, 1), 53)).is_ok() {
            if let Ok(SocketAddr::V4(address)) = socket.local_addr() {
                if usable(*address.ip()) {
                    return Ok(*address.ip());
                }
            }
        }
    }
    // An isolated LAN can have no default Internet route.
    let addresses = if_addrs::get_if_addrs().map_err(|error| error.to_string())?;
    let mut fallback = None;
    for address in addresses {
        if let IpAddr::V4(ip) = address.ip() {
            if !usable(ip) {
                continue;
            }
            if ip.is_private() {
                return Ok(ip);
            }
            fallback.get_or_insert(ip);
        }
    }
    fallback.ok_or_else(|| "No usable LAN IPv4 address; connect this device to a network".into())
}

#[tauri::command]
async fn mobile_start_lan(
    app: tauri::State<'_, MobileApp>,
    port: u16,
) -> Result<MobileStatus, String> {
    let _operation = app.operation.lock().await;
    if port == 0 {
        return Err("Choose a port from 1 to 65535".into());
    }
    if !app.foreground.load(Ordering::Acquire) {
        return Err("Bring the app to the foreground before starting LAN access".into());
    }
    let engine = {
        let mut state = app.state.lock();
        if state
            .service
            .as_ref()
            .is_some_and(|service| service.task.inner().is_finished())
        {
            state.service.take();
        }
        if state.service.is_some() {
            return Err(
                "LAN access is already running; stop it before choosing another port".into(),
            );
        }
        state
            .engine
            .clone()
            .ok_or("Load the model before enabling LAN access")?
    };
    let ip = lan_address()?;
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))
        .await
        .map_err(|error| format!("Cannot bind LAN port {port}: {error}"))?;
    let bound_port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|error| error.to_string())?;
    let token = hex::encode(secret);
    let (stop, stopped) = oneshot::channel();
    let server_token = token.clone();
    let task = tauri::async_runtime::spawn(async move {
        if let Err(error) = lan::serve(listener, engine, server_token, stopped).await {
            eprintln!("LAN listener stopped: {error}");
        }
    });
    if !app.foreground.load(Ordering::Acquire) {
        let _ = stop.send(());
        task.abort();
        return Err("The app moved to the background before LAN access started".into());
    }
    let mut state = app.state.lock();
    if !app.foreground.load(Ordering::Acquire) {
        let _ = stop.send(());
        task.abort();
        return Err("The app moved to the background before LAN access started".into());
    }
    state.service = Some(LanService {
        port: bound_port,
        url: format!("http://{ip}:{bound_port}/"),
        token,
        stop,
        task,
    });
    drop(state);
    Ok(app.status())
}

#[tauri::command]
async fn mobile_stop_lan(app: tauri::State<'_, MobileApp>) -> Result<MobileStatus, String> {
    let _operation = app.operation.lock().await;
    let service = app.state.lock().service.take();
    if let Some(service) = service {
        let _ = service.stop.send(());
        let mut task = service.task;
        if tokio::time::timeout(Duration::from_secs(3), &mut task)
            .await
            .is_err()
        {
            task.abort();
            let _ = task.await;
        }
    }
    Ok(app.status())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(MobileApp {
                data_dir,
                state: StateMutex::new(NativeState::default()),
                operation: tokio::sync::Mutex::new(()),
                foreground: AtomicBool::new(true),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            mobile_status,
            mobile_download_model,
            mobile_load_model,
            mobile_predict,
            mobile_start_lan,
            mobile_stop_lan
        ])
        .build(tauri::generate_context!())
        .expect("initialize Laya mobile")
        .run(|app, event| {
            match event {
                tauri::RunEvent::WindowEvent {
                    event: tauri::WindowEvent::Focused(focused),
                    ..
                } if cfg!(target_os = "android") => {
                    let state = app.state::<MobileApp>();
                    state.foreground.store(focused, Ordering::Release);
                    if !focused {
                        // Conservative stop: never leave the listener reachable after
                        // Android takes this application's window out of focus.
                        state.stop_immediately();
                    }
                }
                tauri::RunEvent::Exit => app.state::<MobileApp>().stop_immediately(),
                _ => {}
            }
        });
}
