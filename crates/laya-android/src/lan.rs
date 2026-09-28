//! Foreground-only LAN access to the on-device inference engine.
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use indexmap::IndexMap;
use laya_inference::{DecisionResult, Error, Request, runtime::Laya};
use serde::Serialize;
use tokio::sync::{Semaphore, oneshot};

const MAX_BODY: usize = 1024 * 1024;
// Laya serializes execution behind its mutex. Admit one executing and one queued
// prediction, rather than filling the blocking thread pool with waiting jobs.
const MAX_PREDICTIONS: usize = 2;

struct AppState {
    engine: Arc<Mutex<Laya>>,
    permits: Arc<Semaphore>,
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

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    let Some(candidate) = value.as_bytes().strip_prefix(b"Bearer ") else {
        return false;
    };
    // Compare token contents without short-circuiting on a matching prefix.
    let mut difference = candidate.len() ^ token.len();
    for (index, expected) in token.bytes().enumerate() {
        difference |= usize::from(candidate.get(index).copied().unwrap_or_default() ^ expected);
    }
    difference == 0
}

async fn require_bearer(
    State(token): State<Arc<str>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let mut response = if authorized(request.headers(), &token) {
        next.run(request).await
    } else {
        error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Bearer token required",
        )
        .into_response()
    };
    // Never cache authenticated responses (including errors) in a shared browser.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn secure_response(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'",
        ),
    );
    response
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    model: &'static str,
    provider: String,
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Health> {
    Json(Health {
        status: "ok",
        model: "multilingual",
        provider: state.provider.clone(),
    })
}

fn decode_request(request: Result<Json<Request>, JsonRejection>) -> Result<Request, HttpError> {
    request.map(|Json(request)| request).map_err(|rejection| {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "body_too_large",
                "Prediction request exceeds 1 MiB",
            )
        } else {
            error(
                StatusCode::BAD_REQUEST,
                "invalid_json",
                "Expected a valid prediction request",
            )
        }
    })
}

async fn predict(
    State(state): State<Arc<AppState>>,
    request: Result<Json<Request>, JsonRejection>,
) -> Result<Json<IndexMap<String, DecisionResult>>, HttpError> {
    let request = decode_request(request)?;
    let permit = state.permits.clone().try_acquire_owned().map_err(|_| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "overloaded",
            "Prediction service is busy",
        )
    })?;
    let engine = Arc::clone(&state.engine);
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit; // Holds capacity through inference, including mutex wait.
        engine
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

async fn index() -> Html<&'static str> {
    Html(include_str!("../assets/index.html"))
}

async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../assets/app.css"),
    )
}

async fn javascript() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../assets/app.js"),
    )
}

async fn mobile_javascript() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../assets/mobile.js"),
    )
}

pub async fn serve(
    listener: tokio::net::TcpListener,
    engine: Arc<Mutex<Laya>>,
    token: String,
    stop: oneshot::Receiver<()>,
) -> std::io::Result<()> {
    let provider = engine
        .lock()
        .map_err(|_| std::io::Error::other("engine lock poisoned"))?
        .provider()
        .to_owned();
    let state = Arc::new(AppState {
        engine,
        permits: Arc::new(Semaphore::new(MAX_PREDICTIONS)),
        provider,
    });
    let api = Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/predictions", post(predict))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .route_layer(middleware::from_fn_with_state(
            Arc::<str>::from(token),
            require_bearer,
        ));
    let app = Router::new()
        .route("/", get(index))
        .route("/app.css", get(css))
        .route("/app.js", get(javascript))
        .route("/mobile.js", get(mobile_javascript))
        .merge(api)
        .with_state(state)
        .layer(middleware::from_fn(secure_response));
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = stop.await;
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request as HttpRequest,
    };
    use tower::ServiceExt;

    fn auth_test_router() -> Router {
        Router::new()
            .route("/api/v1/health", get(|| async { "private" }))
            .route_layer(middleware::from_fn_with_state(
                Arc::<str>::from("secret-token"),
                require_bearer,
            ))
    }

    #[tokio::test]
    async fn missing_or_incorrect_bearer_cannot_reach_api() {
        for credential in [
            None,
            Some("secret-token"),
            Some("Basic secret-token"),
            Some("Bearer wrong-token"),
            Some("Bearer secret-token extra"),
        ] {
            let mut request = HttpRequest::builder().uri("/api/v1/health");
            if let Some(credential) = credential {
                request = request.header(header::AUTHORIZATION, credential);
            }
            let response = auth_test_router()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let body = to_bytes(response.into_body(), MAX_BODY).await.unwrap();
            assert!(
                std::str::from_utf8(&body)
                    .unwrap()
                    .contains("\"code\":\"unauthorized\"")
            );
            assert!(
                !body
                    .windows(b"secret-token".len())
                    .any(|bytes| bytes == b"secret-token")
            );
        }
        let response = auth_test_router()
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/v1/health")
                    .header(header::AUTHORIZATION, "Bearer secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let mut duplicate = HttpRequest::builder()
            .uri("/api/v1/health")
            .header(header::AUTHORIZATION, "Bearer secret-token")
            .body(Body::empty())
            .unwrap();
        duplicate.headers_mut().append(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer secret-token"),
        );
        assert_eq!(
            auth_test_router()
                .oneshot(duplicate)
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }

    async fn validate_request(
        request: Result<Json<Request>, JsonRejection>,
    ) -> Result<StatusCode, HttpError> {
        decode_request(request)?;
        Ok(StatusCode::NO_CONTENT)
    }

    #[tokio::test]
    async fn malformed_and_oversized_prediction_inputs_are_rejected() {
        let app = Router::new()
            .route("/api/v1/predictions", post(validate_request))
            .layer(DefaultBodyLimit::max(MAX_BODY))
            .route_layer(middleware::from_fn_with_state(
                Arc::<str>::from("secret-token"),
                require_bearer,
            ));
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/v1/predictions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(b"{".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        for (body, expected, code) in [
            (b"{".to_vec(), StatusCode::BAD_REQUEST, "invalid_json"),
            (
                br#"{"state":42,"questions":{}}"#.to_vec(),
                StatusCode::BAD_REQUEST,
                "invalid_json",
            ),
            (
                vec![b' '; MAX_BODY + 1],
                StatusCode::PAYLOAD_TOO_LARGE,
                "body_too_large",
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    HttpRequest::builder()
                        .method("POST")
                        .uri("/api/v1/predictions")
                        .header(header::AUTHORIZATION, "Bearer secret-token")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            let content = to_bytes(response.into_body(), MAX_BODY).await.unwrap();
            assert!(std::str::from_utf8(&content).unwrap().contains(code));
        }
    }
}
