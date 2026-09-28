# Local HTTP API

The CLI server loads one model at startup and serves inference plus metadata-only monitoring. The GUI Client serves its interactive page and forwards browser predictions to the CLI server; the GUI Server is a separate administrative application that controls a locally spawned CLI process. All three bind to loopback by default. No authentication is provided: **do not bind to a public interface** without an authenticating reverse proxy and access controls. Clients should not cache live monitoring responses. The inference contract is `/api/v1/`; breaking changes require a new path version.

| Method | URL | Request | Success | Errors |
|---|---|---|---|---|
| GET | `/api/v1/health` (CLI server) | none | 200 `{ "status":"ok", "model":"multilingual", "provider":"cpu" }` | server unavailable |
| GET | `/api/v1/stats` (CLI server) | none | 200 `{ "total_requests":0, "succeeded":0, "failed":0, "average_latency_ms":0.0 }` | 500 telemetry unavailable |
| GET | `/api/v1/requests` (CLI server) | `limit` 1–100 (default 50), optional `cursor` from the preceding page | 200 `{ "items":[{"id":1,"at_unix_ms":0,"status":"ok","duration_ms":35,"question_count":2}], "next_cursor":null }` | 400 invalid query; 500 telemetry unavailable |
| POST | `/api/v1/predictions` (CLI server and GUI Client) | JSON `{"state":"...","questions":{"result":{"type":"noul","instructions":"...","criteria":{}}}}` | 200 object keyed by question ID, values are existing typed `DecisionResult` JSON | 400 invalid JSON/shape; 422 invalid question; 500 inference failure; 502 GUI Client upstream unavailable |

`type` is `choice` (criteria: ordered object of label to description), `score` (criteria: ordered array of levels), or `noul` (criteria: optional empty object). Multiple questions are batched into one ONNX forward pass; question IDs and choices retain insertion order. Request body max 1 MiB; avoid putting secrets in state. POST performs stateless inference on a reused engine and is safe to retry. The GUI Client forwards the same request to the CLI server; API URL is configured by its operator, not supplied by the browser. Errors use `{ "error": { "code": "invalid_json", "message": "..." } }` and non-2xx HTTP statuses; internal paths/stack traces are not exposed.

Monitoring is in-memory and resets when the CLI server restarts. Counters include completed prediction attempts, including malformed JSON and failed inference. Only the latest 100 request records are retained, newest first; cursors refer to request IDs and pages may lose entries as the ring buffer advances. The records contain **only** a timestamp, status, latency and question count—not state, instructions or criteria. Use the returned `next_cursor` for pagination; `null` means no older records remain.

## Local administrator API

The GUI Server Tauri application also serves a separate dashboard on `127.0.0.1:8081` (configurable with `--bind`, loopback only). Its fixed `--api-bind` (default `127.0.0.1:3000`) is reserved for a CLI inference subprocess that it starts; it never takes ownership of another process. JSON administration endpoints:

| Method | URL | Request | Success | Errors |
|---|---|---|---|---|
| GET | `/admin/status` | none | 200 `{ "state":"stopped"|"starting"|"running"|"failed", "model":null|"multilingual"|"english"|"typed-decisions", "device":null|"auto"|"cpu"|"cuda", "pid":null|integer, "uptime_seconds":null|integer, "health":null|{...}, "message":null|string }` | 403 invalid Host |
| POST | `/admin/start` | JSON `{ "model":"multilingual", "device":"cpu" }` | 202 `{ "state":"starting" }`; poll `/admin/status` until running or failed | 400 invalid input/binary; 409 already running or API port in use; 500 spawn failure |
| POST | `/admin/stop` | none | 200 `{ "state":"stopped" }` | 409 no owned process; 500 stop failure |
| GET | `/admin/telemetry` | none | 200 `{ "stats":{...}, "history":{"items":[...],"next_cursor":null} }`; history is latest 50 metadata records | 503 no owned process; 502 API unavailable |

The administrator checks the request Host and rejects cross-origin POST requests; it rejects non-loopback bind addresses. This is **not a substitute for authentication**. Model download/initialization may take time after `/admin/start`; a 202 response does not mean readiness. Errors use the same `{ "error": { "code": "...", "message": "..." } }` shape. `/admin/telemetry` displays statistics from the owned CLI process rather than recording a duplicate copy.

Example:

```bash
curl -sS -X POST http://127.0.0.1:3000/api/v1/predictions -H 'content-type: application/json' -d '{"state":"Please cancel my account","questions":{"cancel":{"type":"noul","instructions":"Does the user want to cancel?","criteria":{}}}}'
```
