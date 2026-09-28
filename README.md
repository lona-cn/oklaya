# laya-rs

**English** | [简体中文](README.zh-CN.md)

Local typed decisions using Rust, Hugging Face `tokenizers` and ONNX Runtime (`ort`). No Python runtime. The default multilingual checkpoint is a [pinned third-party conversion](docs/model-source.md) of the official Laya weights; inspect its [inference contract](docs/inference-contract.md) before gating decisions on confidence.

## Build and run

```bash
cargo build --release
./target/release/laya model download multilingual
./target/release/laya --device cpu bool --text "Please cancel my account" --question "Does the user want to cancel?"
./target/release/laya --device cuda choice --text "The app crashes" --question "Which team?" --option billing=payments --option technical=bugs
```

On Windows use `.\\target\\release\\laya.exe` instead of `./target/release/laya`. CUDA on an RTX 5060 Ti requires the [tested Windows CUDA setup](docs/windows-cuda.md); an explicit `--device cuda` request fails if CUDA cannot execute. `--device auto` attempts CUDA and reports CPU fallback on stderr. `--model english` or `--model typed-decisions` selects a different pinned graph. `model list`, `model path multilingual` and `model download english` manage the persistent per-user cache (`~/.cache/laya-rs/` on Linux; platform-standard cache directory on Windows/macOS). Downloads resume incomplete files, verify SHA256 and atomically promote completed artifacts.

The first call may download hundreds of MB and take time to initialize. Diagnostics, progress and model/provider/path go to stderr; predictions are JSON on stdout.

For the short `laya` commands below, run `cargo install --path crates/laya-cli-server --bin laya` once or put `target/release` on `PATH`; otherwise replace `laya` with the executable path shown above.

## Workspace and desktop apps

| Crate | Role |
|---|---|
| `laya-inference` | Reusable model download, tokenizer and ONNX inference library (`laya_inference`) |
| `laya-cli-server` | `laya` / `laya-inspect` CLI tools and the inference HTTP API, including metadata-only monitoring |
| `laya-gui-server` | Separate Tauri administrator: start/stop a managed inference process, inspect health, counters and recent request metadata |
| `laya-gui-client` | Tauri decision workspace **and** browser-accessible local web server with same-origin prediction proxy |

Build all binaries, then launch the two graphical applications in separate terminals:

```bash
cargo build --workspace --release
./target/release/laya-gui-server
./target/release/laya-gui-client
```

The administrator opens a desktop window and serves its dashboard at `http://127.0.0.1:8081`; choose a model/device and start the inference service there. It looks for the `laya` binary beside its own executable; use `--laya-bin` to point to another copy. The client opens a separate desktop window and also serves its decision page at `http://127.0.0.1:8080`. It forwards predictions to the inference API at `http://127.0.0.1:3000`. For browser-only use, pass `--no-window` to either GUI binary. You can instead start the CLI API manually with `laya --device cpu serve --bind 127.0.0.1:3000`; the administrator cannot stop a process it did not start.

Both graphical interfaces detect the browser or desktop WebView's language on first use: Chinese locales use 简体中文, and other locales use English. Use the language selector in either header to switch at any time; each interface remembers its choice in that browser/WebView.

The administrator shows only in-memory request metadata (timestamp, status, latency, question count), never submitted state or instructions; history is capped at 100 entries and resets on service restart. On Windows, the desktop windows need WebView2; other platforms need their Tauri WebView dependencies. All services bind loopback by default and have no authentication: do not expose them publicly without access controls. API contract and curl example: [HTTP API](docs/http-api.md), [OpenAPI](docs/openapi.yaml). On Windows use `.exe` paths.

## GitHub releases

Push a version tag such as `v0.1.0` to run the [Windows x64 release workflow](.github/workflows/release.yml). It builds and tests all crates, verifies a separately published multilingual model against the pinned hashes, runs the model parity test, and publishes a ZIP of the four executables with project and dependency license notices. `workflow_dispatch` runs the same checks and uploads a temporary build artifact without publishing a version release.

Large model files are **not** in Git. The [pinned model Release](https://github.com/lona-cn/oklaya/releases/tag/models-1bc2622) holds separate `laya-model-{english,multilingual,typed-decisions}.zip` archives, each containing its model directory and Apache license/attribution. Verify their SHA256 checksums before use. These archives are not installed automatically by extracting the application ZIP: by default `laya` downloads and verifies the selected model into its per-user cache, or library callers can supply an extracted directory via `Laya::builder().model_path(...)`. The application release is CPU-ready; CUDA needs separately installed vendor components. The optional original Microsoft ONNX Runtime GPU wheel is mirrored with its own license and third-party notices, but NVIDIA's cuBLAS SDK archive is **not** republished because its distribution terms restrict standalone redistribution. See [Windows CUDA setup](docs/windows-cuda.md) for official downloads.

## CLI

```bash
laya choice --text "The app crashes" --question "Which team?" --option billing="payments and refunds" --option technical="bugs and crashes"
laya bool --text "Please cancel" --question "Does the user want to cancel?"
laya score --text "Production database down" --question "Urgency?" --level low --level medium --level high
laya inspect /path/to/model.onnx
laya-inspect /path/to/model.onnx
laya bench --repetitions 3
```

`score` is the probability-weighted, zero-based level index and includes a level legend; `bool` uses the Laya `noul` primitive and returns P(true). Every result includes the independent decision-head `action_probability`. The entropy `confidence` is distinct from the top-answer `answer_confidence`. No universal deployment threshold is implied.

## JSON mode and library

```json
{"state":"Production database is unavailable.","questions":{"urgency":{"type":"score","instructions":"How urgent?","criteria":["low","medium","high"]},"outage":{"type":"noul","instructions":"Is this an outage?"}}}
```

Save as `request.json` then run `laya --device cpu predict request.json`, or pipe to `laya predict -`. Output is a JSON object keyed by question ID; one forward pass covers all questions, one tokenizer and session are reused within an engine. Integration libraries use `Laya::builder().model(ModelKind::Multilingual).device(Device::Cpu).build()?` then `engine.predict(state, &questions)?`, where questions are ordered `IndexMap<String, Question>` and results are strongly typed `DecisionResult` values. `--max-len 8192` opts into longer multilingual context; state truncation is reported, not silently ignored.

## Verification

`cargo test --workspace` runs unit and HTTP proxy tests without downloading a model. Set `LAYA_RUN_MODEL_TESTS=1` and run `cargo test -p laya-inference --test reference` for the 23-case official Python ONNXAgent differential test and token-by-token tokenizer test; the suite then downloads a pinned model unless `LAYA_MODEL_DIR` points to a preexisting model folder. To inspect the actual input/output schema use `laya-inspect`. Benchmark reports measured warmup, model load, p50, p95 and throughput at 1/5/10/50 questions; numbers are hardware-specific.

Model provenance, hashes and caveats: [model source](docs/model-source.md). Token/prompt/decision details: [inference contract](docs/inference-contract.md). GPU installation: [Windows CUDA](docs/windows-cuda.md).

## License

Project source code and original assets are licensed under the [Apache License 2.0](LICENSE), consistent with the crate manifests. Downloaded Laya model weights and ONNX conversions are separate Apache-2.0 works: the original weights are credited to Convai Innovations, and the ONNX exports are published by codenamev. These artifacts are not included in Git or relicensed by this project. See [NOTICE](NOTICE) and [model provenance](docs/model-source.md). Third-party libraries and native runtimes keep their own licenses (including MIT, MPL-2.0 and Unicode-3.0 dependencies); binary redistributors must retain the applicable license and attribution notices for what they bundle.
