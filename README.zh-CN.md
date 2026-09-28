# laya-rs

[English](README.md) | **简体中文**

使用 Rust、Hugging Face `tokenizers` 和 ONNX Runtime（`ort`）在本地执行类型化决策推理，无需 Python 运行时。默认的 multilingual 模型是基于 Laya 官方权重的[第三方 ONNX 转换版本](docs/model-source.md)，版本已固定。若要依据置信度作自动决策，请先阅读[推理契约](docs/inference-contract.md)。

## 构建与运行

```bash
cargo build --release
./target/release/laya model download multilingual
./target/release/laya --device cpu bool --text "Please cancel my account" --question "Does the user want to cancel?"
./target/release/laya --device cuda choice --text "The app crashes" --question "Which team?" --option billing=payments --option technical=bugs
```

Windows 上请将 `./target/release/laya` 换成 `.\target\release\laya.exe`。RTX 5060 Ti 的 CUDA 推理需要按照[已验证的 Windows CUDA 安装说明](docs/windows-cuda.md)配置；显式指定 `--device cuda` 而 CUDA 无法运行时，命令会报错。`--device auto` 会尝试 CUDA，并在回退 CPU 时将原因写入 stderr。`--model english` 和 `--model typed-decisions` 可选择其他固定版本的模型。`model list`、`model path multilingual`、`model download english` 用于管理用户级持久缓存（Linux 为 `~/.cache/laya-rs/`，Windows/macOS 使用平台标准缓存目录）。下载支持续传，完成后校验 SHA256，再原子地替换目标文件。

首次运行可能需要下载数百 MB 的文件并初始化模型。诊断信息、进度及模型、执行提供程序、文件路径写入 stderr；预测 JSON 写入 stdout。

以下示例中的简写命令 `laya` 可通过 `cargo install --path crates/laya-cli-server --bin laya` 安装，或将 `target/release` 加入 `PATH`；也可以改用上方的可执行文件路径。

## Workspace 与应用

| Crate | 职责 |
|---|---|
| `laya-inference` | 可复用的模型下载、tokenizer 和 ONNX 推理库（`laya_inference`） |
| `laya-cli-server` | `laya` / `laya-inspect` 命令、推理 HTTP API 和仅含请求元数据的监控接口 |
| `laya-gui-server` | 独立的 Tauri 图形化管理后台：启动或停止由其管理的推理进程，查看健康状态、计数和近期请求元数据 |
| `laya-gui-client` | Tauri 决策工作台，同时提供可由浏览器访问的本地网页服务和同源预测代理 |
| `laya-android` | 独立 Android ARM64 应用：设备本地 CPU ONNX 推理、私有模型存储、可选端口的受保护局域网服务及专属中英双语测试页面 |

构建桌面二进制文件后，在两个终端分别启动两个图形应用：

```bash
cargo build --workspace --release
./target/release/laya-gui-server
./target/release/laya-gui-client
```

管理后台打开桌面窗口，并在 `http://127.0.0.1:8081` 提供网页；在其中选择模型和设备，再启动推理服务。它默认查找与自身可执行文件同目录的 `laya`；如存放在别处，可通过 `--laya-bin` 指定。客户端打开独立桌面窗口，也在 `http://127.0.0.1:8082` 提供决策页面；预测请求转发至 `http://127.0.0.1:3000` 的推理 API。只用浏览器时，可给任一 GUI 程序添加 `--no-window`。也可手动运行 `laya --device cpu serve --bind 127.0.0.1:3000` 启动推理 API；管理后台不能停止并非由其启动的进程。

两个图形界面首次打开时会根据浏览器或桌面 WebView 的语言自动选择简体中文（中文语言环境）或英文（其他语言环境）。可随时通过各自页面顶部的语言选择框切换；所选语言会保存在该浏览器或 WebView 中。

管理后台仅显示内存中的请求元数据（时间、状态、延迟、问题数量），不会记录提交的状态文本或指令；历史最多保留 100 条，服务重启后清空。Windows 桌面窗口需要 WebView2；其他平台需要安装 Tauri 所依赖的 WebView 组件。桌面服务默认只监听本机，且没有身份验证；如需对外开放，必须增加访问控制。接口及 curl 示例见 [HTTP API](docs/http-api.md) 和 [OpenAPI](docs/openapi.yaml)。Windows 上请使用相应的 `.exe` 文件。

## Android 应用

独立的 [`laya-android` crate](docs/android.md) 可构建 ARM64 Android APK，**不依赖 `laya-gui-client` 或桌面/CLI 推理服务**。在手机中下载并加载已固定版本的 multilingual 模型后，即可本机推理；也可选择局域网端口，启动服务，在另一台设备的浏览器中打开手机显示的地址并输入本次启动生成的 Bearer 令牌，从测试页面提交问题。手机失焦时会停止服务；局域网 HTTP **未加密**，只能用于可信网络。构建安装、操作、安全限制以及 Vulkan/WebGPU 可行性调研见 [Android 指南](docs/android.md)。目前实际执行后端为 CPU，不声称具备 Vulkan 加速。

## GitHub 发布

修改 Android 构建输入的 PR 或 `master` 分支提交会在[发布工作流](.github/workflows/release.yml)中构建 ARM64 APK。配置 [Android 签名 Secrets](docs/android.md#ci-and-github-releases) 后推送 `v0.2.0` 版本标签：Windows x64 构建测试、固定模型校验与差分测试和 Android 签名发布包并行进行；两个平台都通过后，由唯一的发布任务将四个 Windows 可执行文件的 ZIP、可安装的 ARM64 APK 和合并的 SHA256 校验文件发布到**同一** GitHub Release。手动运行 `workflow_dispatch` 只上传临时 Windows/Android 构建产物，不发布版本。APK 不包含模型权重，首次使用由手机自行下载。

大模型文件**不进入 Git**。[固定版本的模型 Release](https://github.com/lona-cn/oklaya/releases/tag/models-1bc2622) 分别提供 `laya-model-{english,multilingual,typed-decisions}.zip`，每个压缩包包含模型目录以及 Apache 许可和归属声明。使用前应校验 SHA256。仅解压应用程序 ZIP 不会自动安装这些模型：默认情况下，`laya` 会把选中的模型下载并校验到用户缓存；推理库调用者也可以通过 `Laya::builder().model_path(...)` 指定已解压目录。应用发行包可用于 CPU；CUDA 需另行安装供应商组件。可选的微软 ONNX Runtime GPU 原版 wheel 连同自带许可和第三方声明上传；NVIDIA cuBLAS SDK 压缩包的分发条款限制单独转载，因此**不会**镜像发布，获取方式见 [Windows CUDA 指南](docs/windows-cuda.md)。

## CLI

```bash
laya choice --text "The app crashes" --question "Which team?" --option billing="payments and refunds" --option technical="bugs and crashes"
laya bool --text "Please cancel" --question "Does the user want to cancel?"
laya score --text "Production database down" --question "Urgency?" --level low --level medium --level high
laya inspect /path/to/model.onnx
laya-inspect /path/to/model.onnx
laya bench --repetitions 3
```

`score` 返回按概率加权的、从零开始的等级索引，并附带等级说明；`bool` 使用 Laya 的 `noul` 类型，返回 P(true)。每个结果都包含独立决策头的 `action_probability`。基于熵计算的 `confidence` 与最高答案概率 `answer_confidence` 并不相同；项目不提供适用于所有场景的统一阈值。

## JSON 模式与推理库

```json
{"state":"Production database is unavailable.","questions":{"urgency":{"type":"score","instructions":"How urgent?","criteria":["low","medium","high"]},"outage":{"type":"noul","instructions":"Is this an outage?"}}}
```

保存为 `request.json`，运行 `laya --device cpu predict request.json`；也可将 JSON 通过 stdin 传入 `laya predict -`。输出是以问题 ID 为键的 JSON 对象。多个问题共用一次模型前向计算；同一个推理引擎复用 tokenizer 和 ONNX session。库调用方式为 `Laya::builder().model(ModelKind::Multilingual).device(Device::Cpu).build()?`，随后调用 `engine.predict(state, &questions)?`；其中 `questions` 是有序的 `IndexMap<String, Question>`，结果为强类型 `DecisionResult`。`--max-len 8192` 可启用更长的 multilingual 上下文；状态文本被截断时会报告，而非静默忽略。

## 验证

`cargo test --workspace` 会运行单元测试和 HTTP 代理测试，不下载模型。设置 `LAYA_RUN_MODEL_TESTS=1` 并运行 `cargo test -p laya-inference --test reference`，会执行包含 23 个用例的官方 Python ONNXAgent 差分测试和逐 token 的 tokenizer 测试；如未通过 `LAYA_MODEL_DIR` 指定已有模型目录，测试可能下载已固定版本的模型。使用 `laya-inspect` 可检查实际模型的输入输出结构。基准测试报告模型加载时间、预热时间，以及单次处理 1/5/10/50 个问题时的 p50、p95 和吞吐率；结果取决于运行设备。

模型来源、哈希和限制见[模型来源](docs/model-source.md)；token、提示词和决策细节见[推理契约](docs/inference-contract.md)；GPU 安装见 [Windows CUDA 指南](docs/windows-cuda.md)。

## 许可证

本项目源代码及原创资源采用 [Apache License 2.0](LICENSE)，与各 crate 的许可声明一致。下载的 Laya 模型权重和 ONNX 转换文件是独立的 Apache-2.0 作品：原始权重归属 Convai Innovations，ONNX 转换版本由 codenamev 发布。这些文件不在 Git 仓库中，也不会因本项目的许可证而被重新授权。来源及归属见 [NOTICE](NOTICE) 和[模型来源](docs/model-source.md)。第三方库和原生运行时仍适用各自的许可证（依赖中包括 MIT、MPL-2.0 和 Unicode-3.0）；发布包含这些组件的二进制文件时，应保留所包含组件适用的许可和归属声明。
