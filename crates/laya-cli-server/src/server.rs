use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    routing::{get, post},
};
use indexmap::IndexMap;
use laya_inference::{DecisionResult, Error, Request, runtime::Laya};
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::telemetry::{RequestPage, Stats, Telemetry};

struct AppState {
    engine: Mutex<Laya>,
    telemetry: Mutex<Telemetry>,
    model: String,
    provider: String,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    model: String,
    provider: String,
}

#[derive(Serialize)]
struct ApiError {
    error: ErrorBody,
}
#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

type HttpError = (StatusCode, Json<ApiError>);

fn error(status: StatusCode, code: &'static str, message: &'static str) -> HttpError {
    (
        status,
        Json(ApiError {
            error: ErrorBody { code, message },
        }),
    )
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Health> {
    Json(Health {
        status: "ok",
        model: state.model.clone(),
        provider: state.provider.clone(),
    })
}

#[derive(Deserialize)]
struct RequestQuery {
    limit: Option<usize>,
    cursor: Option<String>,
}

async fn stats(State(state): State<Arc<AppState>>) -> Result<Json<Stats>, HttpError> {
    let telemetry = state.telemetry.lock().map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "telemetry_failed",
            "Monitoring data unavailable",
        )
    })?;
    Ok(Json(telemetry.stats()))
}

async fn requests(
    State(state): State<Arc<AppState>>,
    query: Result<Query<RequestQuery>, QueryRejection>,
) -> Result<Json<RequestPage>, HttpError> {
    let Query(query) = query.map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Invalid pagination parameters",
        )
    })?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Limit must be between 1 and 100",
        ));
    }
    let cursor = query
        .cursor
        .map(|value| value.parse::<u64>())
        .transpose()
        .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_query", "Invalid cursor"))?;
    let telemetry = state.telemetry.lock().map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "telemetry_failed",
            "Monitoring data unavailable",
        )
    })?;
    Ok(Json(telemetry.page(limit, cursor)))
}

async fn predict(
    State(state): State<Arc<AppState>>,
    request: Result<Json<Request>, JsonRejection>,
) -> Result<Json<IndexMap<String, DecisionResult>>, HttpError> {
    let start = Instant::now();
    let question_count = request
        .as_ref()
        .map(|Json(request)| request.questions.len())
        .unwrap_or(0);
    let result = predict_inner(state.clone(), request).await;
    let mut telemetry = state.telemetry.lock().map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "telemetry_failed",
            "Monitoring data unavailable",
        )
    })?;
    telemetry.record(result.is_ok(), start.elapsed(), question_count);
    result
}

async fn predict_inner(
    state: Arc<AppState>,
    request: Result<Json<Request>, JsonRejection>,
) -> Result<Json<IndexMap<String, DecisionResult>>, HttpError> {
    let Json(request) = request.map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Expected a valid prediction request",
        )
    })?;
    let engine = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        engine
            .engine
            .lock()
            .map_err(|_| Error::Inference("engine lock poisoned".into()))?
            .predict(&request.state, &request.questions)
    })
    .await
    .map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "inference_failed",
            "Inference failed",
        )
    })?;
    match result {
        Ok(decisions) => Ok(Json(decisions)),
        Err(Error::InvalidQuestion(_) | Error::InvalidModelInput(_)) => Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_question",
            "Invalid question or model input",
        )),
        Err(_) => Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "inference_failed",
            "Inference failed",
        )),
    }
}

pub async fn serve(addr: SocketAddr, model: String, engine: Laya) -> anyhow::Result<()> {
    let provider = engine.provider().to_owned();
    let state = Arc::new(AppState {
        engine: Mutex::new(engine),
        telemetry: Mutex::new(Telemetry::default()),
        model,
        provider,
    });
    let app = Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/requests", get(requests))
        .route("/api/v1/predictions", post(predict))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("API listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
