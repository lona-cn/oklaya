use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use clap::Parser;
use tauri::{RunEvent, WebviewUrl, WebviewWindowBuilder};

const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const PREDICTIONS_PATH: &str = "/api/v1/predictions";

#[derive(Parser)]
#[command(about = "Open the Laya workspace or serve it in a browser")]
struct Args {
    /// Interface to listen on (bind to a private interface only if you trust its clients).
    #[arg(long, default_value = "127.0.0.1:8082")]
    bind: SocketAddr,
    /// Base URL of the Laya inference API.
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    api_url: reqwest::Url,
    /// Serve the browser workspace without opening a desktop window.
    #[arg(long)]
    no_window: bool,
}

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    predictions_url: reqwest::Url,
}

fn predictions_url(mut base: reqwest::Url) -> Result<reqwest::Url, &'static str> {
    if !matches!(base.scheme(), "http" | "https") || base.host().is_none() {
        return Err("--api-url must be an HTTP(S) URL with a host");
    }
    base.set_path(PREDICTIONS_PATH);
    base.set_query(None);
    base.set_fragment(None);
    Ok(base)
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.css", get(css))
        .route("/app.js", get(js))
        .route(PREDICTIONS_PATH, post(proxy_prediction))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(state)
}

async fn index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        laya_gui_client::INDEX_HTML,
    )
}

async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        laya_gui_client::APP_CSS,
    )
}

async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        laya_gui_client::APP_JS,
    )
}

fn upstream_unavailable() -> Response {
    (
        StatusCode::BAD_GATEWAY,
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"error":{"code":"upstream_unavailable","message":"The inference service is unavailable. Please try again later."}}"#,
    )
        .into_response()
}

async fn proxy_prediction(State(state): State<AppState>, body: Bytes) -> Response {
    let upstream = match state
        .client
        .post(state.predictions_url.clone())
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return upstream_unavailable(),
    };
    let status = upstream.status();
    let content_type = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| header::HeaderValue::from_static("application/json"));
    match upstream.bytes().await {
        Ok(payload) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(payload))
            .expect("valid upstream status and content type"),
        Err(_) => upstream_unavailable(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let state = AppState {
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()?,
        predictions_url: predictions_url(args.api_url)?,
    };
    // Bind before opening the window: a port conflict is a startup error, not a blank UI.
    let listener = tauri::async_runtime::block_on(tokio::net::TcpListener::bind(args.bind))?;
    let address = listener.local_addr()?;
    println!("Laya workspace listening on http://{address}");
    let app = router(state);

    if args.no_window {
        tauri::async_runtime::block_on(async { axum::serve(listener, app).await })?;
        return Ok(());
    }

    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server = tauri::async_runtime::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await
    });
    let window_address = if address.ip().is_unspecified() {
        let loopback = match address.ip() {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
        };
        SocketAddr::new(loopback, address.port())
    } else {
        address
    };
    let url = reqwest::Url::parse(&format!("http://{window_address}/"))?;
    let desktop = tauri::Builder::default()
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Laya — Mission brief")
                .inner_size(1200.0, 850.0)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())?;
    let mut stop_tx = Some(stop_tx);
    let mut server = Some(server);
    desktop.run(move |_, event| {
        if let RunEvent::Exit = event {
            if let Some(tx) = stop_tx.take() {
                let _ = tx.send(());
            }
            if let Some(mut task) = server.take() {
                // Graceful first; abort an active browser request rather than keep the
                // listener alive indefinitely after the desktop window closes.
                tauri::async_runtime::block_on(async {
                    if tokio::time::timeout(Duration::from_secs(3), &mut task)
                        .await
                        .is_err()
                    {
                        task.abort();
                        let _ = task.await;
                    }
                });
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_target_is_fixed_to_predictions() {
        let target = predictions_url(
            "http://127.0.0.1:3000/custom?target=evil#fragment"
                .parse()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(target.as_str(), "http://127.0.0.1:3000/api/v1/predictions");
        assert!(predictions_url("file:///tmp/other".parse().unwrap()).is_err());
    }

    #[tokio::test]
    async fn unreachable_upstream_returns_safe_json_error() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let unavailable = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let response = proxy_prediction(
            State(AppState {
                client: reqwest::Client::new(),
                predictions_url: predictions_url(unavailable.parse().unwrap()).unwrap(),
            }),
            Bytes::from_static(b"{}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["code"], "upstream_unavailable");
        assert!(
            !body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("127.0.0.1")
        );
    }
}
