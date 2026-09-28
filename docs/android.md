# Android app (ARM64)

`crates/laya-android` is a standalone Tauri 2 Android application. It uses `laya-inference` directly, with its **own** UI and LAN server; it neither starts nor depends on `laya-gui-client` or the CLI server. The app loads the pinned multilingual ONNX model on the device with ONNX Runtime **CPU**. The model is downloaded on first use into this app's private data directory and verified against the pinned SHA-256 digests before loading. The model graph alone is about 647 MB (decimal); allow additional space for the tokenizer, download staging, ONNX initialization and inference memory. The APK does not bundle model weights.

## Build and install

Install the Rust Android ARM64 target, Android SDK (platform 36), NDK, Java and Tauri CLI 2. Set `ANDROID_HOME`, `NDK_HOME` and `JAVA_HOME` for your local installation. The checked-in Android project is under `crates/laya-android/gen/android` (package `dev.laya.mobile`, minimum SDK 24, ARM64 build target). From the repository root:

```sh
rustup target add aarch64-linux-android
cd crates/laya-android
cargo tauri android build --debug --target aarch64 --apk
```

The **local debug** APK is `gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk` relative to that crate. Install with `adb install -r <apk-path>` on an ARM64 Android device (Android 7.0+, API 24+). It uses the Android **debug key**, which is not suitable for publishing. The command without `--debug` builds the release variant; without the signing environment below, its APK is unsigned and cannot be installed.

On Windows without Developer Mode, the `--debug` command may compile the Rust `.so` successfully but fail at its Gradle step because creating symlinks requires permission. Rather than weakening system policy, after it has compiled `target/aarch64-linux-android/debug/liblaya_android.so`, copy the library into the generated Android project and run Gradle without rebuilding Rust (PowerShell, from the repository root):

```powershell
$jni = 'crates/laya-android/gen/android/app/src/main/jniLibs/arm64-v8a'
New-Item -ItemType Directory -Force $jni | Out-Null
Copy-Item 'target/aarch64-linux-android/debug/liblaya_android.so' "$jni/liblaya_android.so"
Push-Location 'crates/laya-android/gen/android'
./gradlew.bat :app:assembleArm64Debug -x :app:rustBuildArm64Debug --no-daemon
Pop-Location
```

Recompile and recopy the library after **any** native or bundled UI change; Gradle's `-x` switch intentionally skips that step. The standard Tauri build is preferable when symlinks are supported. Set `NDK_HOME` to the **same installed NDK** used for the Rust build: the Gradle project copies that NDK's `libc++_shared.so` into the APK because ONNX Runtime needs it at startup. The JDK must also be available to Gradle.

## CI and GitHub Releases

The [release workflow](../.github/workflows/release.yml) builds an ARM64 debug APK for Android-related pull requests and pushes to `master`; `workflow_dispatch` uploads temporary Android and Windows build artifacts without publishing. Pushing a `v*` version tag builds a **release-signed** ARM64 APK in parallel with the existing Windows ZIP and model parity checks. Only after both builds succeed does one job create a GitHub Release containing `oklaya-<tag>-android-arm64.apk`, `oklaya-<tag>-windows-x64.zip` and a combined `SHA256SUMS.txt`. The release APK is at `gen/android/app/build/outputs/apk/arm64/release/app-arm64-release.apk` before packaging. Models remain separate downloads, not part of the APK.

Before pushing the **first version tag**, create and back up an Android signing keystore outside this repository. Add these repository **Actions secrets** under Settings → Secrets and variables → Actions:

| Secret | Value |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | Base64 of the complete release `.jks` file (single line) |
| `ANDROID_STORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | Alias of its signing key |
| `ANDROID_KEY_PASSWORD` | Password for that key |

For example, obtain the first value with `python -c 'import base64,sys; print(base64.b64encode(open(sys.argv[1],"rb").read()).decode())' /secure/path/laya-release.jks`. **The printed Base64 is the private keystore in reversible form:** put it only in the GitHub secret, not in a commit, issue or log. The workflow refuses tag publication if any secret is missing, if Android marks the APK debuggable, or if it is signed with an Android debug certificate. **Keep the keystore and passwords backed up:** a different signing key cannot update an already installed app with the same package name. GitHub-hosted debug CI does not require these secrets.

## Use

1. Open **Laya Mobile** on the phone. Tap **Download model**, then **Load model**; wait for the provider to display `cpu`. Downloads require Internet and can take time. Subsequent launches reuse the verified private files but must load a new in-memory session.
2. Enter a situation and a question on the phone and tap **Run decision** for fully local inference, without enabling LAN access.
3. To share inference with another device on a **trusted LAN**, choose a free port from 1 to 65535 (initial value 8787) and tap **Start LAN server**. The app binds all local IPv4 interfaces and displays a suggested LAN URL plus a freshly generated bearer token. Allow local-network connections to that port if the phone or access point filters them. A VPN, multiple interfaces, or client isolation may make the suggested URL unreachable; use the phone's reachable IPv4 address on the same Wi-Fi if necessary.
4. On another device open the displayed `http://<phone-ip>:<port>/` in a browser, enter the token shown on the phone, fill in the question and submit. The token stays in that browser tab's memory (only the language preference is stored). The browser uses the same-origin `/api/v1/predictions` endpoint. **Stop LAN server** releases the port and revokes that token; restarting generates a new token.

Keep the app in the foreground with its window focused while sharing. On Android the listener stops when the app window loses focus or the process exits; this app does **not** promise background serving, a foreground service, or persistence through screen lock. The model is not exposed on the LAN as a downloadable file. There is no `/api/v1/stats` or `/api/v1/requests` mobile endpoint.

### LAN HTTP contract and security

The LAN page and its static assets are public so a browser can load the form. Both mobile API endpoints require exactly one `Authorization: Bearer <token>` header. `GET /api/v1/health` returns `{"status":"ok","model":"multilingual","provider":"cpu"}`. `POST /api/v1/predictions` accepts the [existing prediction JSON contract](http-api.md) (1 MiB request body maximum) and returns its keyed decision result. The server rejects missing/wrong tokens with 401, invalid requests with 400/422, oversized bodies with 413, and excess parallel work with 503. It provides no cross-origin access; browser scripts call it from the served page. Responses carry no-store, CSP, no-referrer and nosniff headers.

**HTTP is not encrypted.** Anyone who can observe LAN traffic can read or replay the bearer token and read submitted questions/results. Use only a network whose devices and operators you trust; never expose the port to the Internet or forward it from a router. The random token limits *unauthorized* requests, but does not provide confidentiality, TLS or protection from an observer on the same network.

## Vulkan / GPU feasibility

This implementation intentionally uses CPU rather than claiming Vulkan acceleration:

- The project's pinned `ort` 2.0.0-rc.13 corresponds to ONNX Runtime 1.28. The [ORT 1.28 execution-provider registry](https://github.com/microsoft/onnxruntime/blob/v1.28.0/onnxruntime/core/providers/get_execution_providers.cc) does **not** list a native `VulkanExecutionProvider`; Android Vulkan is not a drop-in provider setting for this build.
- [ORT WebGPU EP](https://onnxruntime.ai/docs/execution-providers/WebGPU-ExecutionProvider.html) may use Dawn's Vulkan backend on supported Android devices. **WebGPU EP is distinct from Vulkan EP**. The [pinned `ort-sys` prebuilt distribution list](https://docs.rs/crate/ort-sys/2.0.0-rc.13/source/build/download/dist.tsv) includes Android ARM64 with NNAPI, not an Android WebGPU-enabled build. The NNAPI feature is available to the Android Rust dependency, but this app does not enable NNAPI in a session or claim an NNAPI performance result.
- A GPU experiment needs a compatible custom ONNX Runtime + Dawn Android ARM64 build, working driver and device support, Rust EP linkage, and actual profiling of this model's integer/dynamic-shape operators for fallback or partitioning. Building or shipping a Vulkan-capable Android GPU path has **not** been verified for this pinned runtime and model. For documented mobile alternatives, see [ORT's Android deployment guide](https://onnxruntime.ai/docs/build/android.html) and its [execution-provider overview](https://onnxruntime.ai/docs/execution-providers/).
