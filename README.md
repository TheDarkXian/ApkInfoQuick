# ApkInfoQuick

[中文](#中文) | [English](#english)

## 中文

ApkInfoQuick 是一个面向 APK/AAB 的桌面信息查看器与命令行解析工具，基于 React、Material UI、Tauri 和 Rust 开发。

它的目标是做一个更现代、更适合日常排查和脚本自动化的 APK 信息工具：GUI 负责快速查看和导出，CLI 负责批量解析、流水线集成和自动化处理。

### 功能特性

- 桌面 GUI：支持拖拽 APK/AAB、多标签、紧凑信息布局、图标预览和图标导出。
- CLI：支持单文件/多文件/目录递归解析，适合脚本和 CI 使用。
- 统一输出契约：GUI 和 CLI 复用同一套 `Envelope + data` JSON 结构。
- APK 解析：优先使用 `aapt dump badging/resources/xmltree`，失败时回退到 Rust 自研解析链路。
- AAB 解析：内置 `bundletool.jar`，通过 universal APK 转换后复用 APK 解析链路。
- 图标提取：结合 aapt 候选、resources.arsc 反查、adaptive icon、启发式扫描等策略。
- 信息覆盖：包名、应用名、版本、SDK、权限、ABI、渠道、签名、图标来源、warnings 和错误信息。
- 交互规则：GUI 不使用浏览器原生 `alert`、`confirm`、`prompt`，统一使用 toast 和内联状态。

### 项目结构

```text
.
|-- backend/              Rust 解析引擎和 CLI
|-- frontend/             React UI 和 Tauri 桌面壳
|-- frontend/templates/   GUI/CLI 共享的文本复制模板
|-- tools/android/        内置 Android 辅助工具，例如 aapt.exe 和 bundletool.jar
|-- AI_PROJECT_CONTEXT.md 项目上下文说明
`-- README.md
```

### GUI 使用

开发模式运行：

```powershell
cd frontend
npm install
npm run tauri:dev
```

构建桌面安装包：

```powershell
cd frontend
npm run tauri:build
```

Tauri 打包会携带 `tools/android/*`，普通用户不需要手动配置 `aapt.exe` 或 `bundletool.jar`。AAB 解析仍需要系统可用的 Java。

### CLI 使用

CLI 位于 `backend`，并复用 GUI 的同一套解析引擎。

```powershell
cd backend
cargo run -- parse path\to\app.apk
```

常用命令：

```powershell
# 默认输出格式化 JSON Envelope
cargo run -- parse path\to\app.apk

# 解析 AAB
cargo run -- parse path\to\app.aab

# 输出单行紧凑 JSON
cargo run -- parse path\to\app.apk --compact

# 输出可读文本，默认复用 frontend/templates/copy-text.template.txt
cargo run -- parse path\to\app.apk --text

# 使用自定义文本模板
cargo run -- parse path\to\app.apk --text --template .\my-template.txt

# 关闭 stderr 进度输出，适合机器消费
cargo run -- parse path\to\app.apk --quiet

# 解析并导出图标到目录
cargo run -- parse path\to\app.apk --export-icon .\icons

# 递归批量解析目录并写入结果文件
cargo run -- parse .\samples --recursive --out result.json

# 检查 CLI 环境和 Android 工具发现情况
cargo run -- doctor
cargo run -- doctor --compact
```

CLI 进度信息默认输出到 stderr，最终 JSON/text 输出到 stdout，因此可以安全用于管道和重定向。使用 `--quiet` 可关闭进度输出。

构建 CLI：

```powershell
cd backend
cargo build --release
```

生成的二进制名称为 `apkinfoquick`。

### CLI 发布包建议

如果单独分发 CLI，建议保持以下结构：

```text
release/
|-- apkinfoquick.exe
`-- tools/
    `-- android/
        |-- aapt.exe
        `-- bundletool.jar
```

AAB 解析依赖 Java。即使发布包内置了 `bundletool.jar`，用户系统仍需要能找到 `java` 或配置 `JAVA_HOME`。

### JSON 契约

所有解析结果使用统一外层 Envelope：

```json
{
  "success": true,
  "data": {
    "packageName": "com.example.app",
    "appName": "Example",
    "iconUrl": "file:///...",
    "minSdkVersion": 23,
    "targetSdkVersion": 35,
    "compileSdkVersion": 35,
    "versionCode": 1,
    "versionName": "1.0.0",
    "permissions": [],
    "signers": [],
    "abis": [],
    "channel": "unknown"
  },
  "errorCode": null,
  "errorMessage": null,
  "warnings": []
}
```

默认值约定：

- `versionName`：缺失时为 `null`。
- `compileSdkVersion`：缺失时为 `null`。
- `channel`：未解析到时为 `"unknown"`。
- `permissions`、`abis`、`signers`：为空时为 `[]`。

### Doctor 输出

`doctor` 用于检查运行环境：

- `apkReady`：APK 解析是否可用。即使 `aapt` 缺失，Rust fallback 仍可工作，因此通常为 `true`。
- `aabReady`：AAB 解析是否可用，需要 `bundletool.jar` 和 Java 同时可用。
- `warnings`：环境降级或缺失项提示。
- `aapt`、`bundletool`、`java`、`toolsDir`：实际发现路径。

### Android 工具查找顺序

`aapt` 查找顺序：

1. Tauri 内置资源：`tools/android/aapt.exe`。
2. 工作区路径：`tools/android/aapt.exe`。
3. `APK_INFO_AAPT` 环境变量。
4. `PATH`。

`bundletool` 查找顺序：

1. Tauri/工作区路径：`tools/android/bundletool.jar`。
2. `APK_INFO_BUNDLETOOL` 环境变量。

更多说明见 [tools/android/README.md](tools/android/README.md)。

### 开发

安装前端依赖：

```powershell
cd frontend
npm install
```

运行前端测试：

```powershell
cd frontend
npm run test -- --run
```

运行前端构建：

```powershell
cd frontend
npm run build
```

运行后端测试：

```powershell
cd backend
cargo test
```

### 当前状态

当前重点：

- APK/AAB 解析与展示。
- GUI 高密度工作流。
- CLI 自动化和批量处理。

后续可能增强：

- 原生 AAB 解析，降低对 bundletool 和 Java 的依赖。
- 更完整的签名验证。
- 更多导出格式。

### 许可证

本仓库暂未声明许可证。正式开源发布前请补充 LICENSE。

内置第三方 Android 工具可能有各自的许可证和再分发要求，发布二进制包前请确认相关条款。

## English

ApkInfoQuick is a desktop APK/AAB metadata viewer and command-line parser built with React, Material UI, Tauri, and Rust.

It aims to be a modern replacement for older APK info tools: the GUI is optimized for inspection and export, while the CLI is designed for batch parsing, automation, and CI workflows.

### Features

- Desktop GUI with drag-and-drop, multi-tab workspace, dense metadata layout, icon preview, and icon export.
- CLI support for single files, multiple files, recursive directory parsing, and scripted workflows.
- Unified `Envelope + data` JSON contract shared by GUI and CLI.
- APK parsing with `aapt dump badging/resources/xmltree` first, then Rust fallback.
- AAB parsing through bundled `bundletool.jar`, converting to a universal APK before reusing the APK parser.
- Robust icon extraction using aapt candidates, resources.arsc reverse lookup, adaptive icons, and heuristic fallback.
- Metadata coverage: package name, app name, versions, SDK levels, permissions, ABI, channel, signatures, icon source, warnings, and errors.
- No browser-native `alert`, `confirm`, or `prompt` in the GUI; feedback uses toast and inline states.

### Project Structure

```text
.
|-- backend/              Rust parser engine and CLI
|-- frontend/             React UI and Tauri desktop shell
|-- frontend/templates/   Shared copy-text templates for GUI/CLI
|-- tools/android/        Bundled Android helper tools, such as aapt.exe and bundletool.jar
|-- AI_PROJECT_CONTEXT.md Project handoff/context notes
`-- README.md
```

### GUI Usage

Run the desktop app in development mode:

```powershell
cd frontend
npm install
npm run tauri:dev
```

Build desktop installers:

```powershell
cd frontend
npm run tauri:build
```

The Tauri bundle includes `tools/android/*`, so end users do not need to configure `aapt.exe` or `bundletool.jar` manually. AAB parsing still requires Java to be available on the system.

### CLI Usage

The CLI lives in `backend` and uses the same parser engine as the GUI.

```powershell
cd backend
cargo run -- parse path\to\app.apk
```

Common commands:

```powershell
# Pretty JSON Envelope by default
cargo run -- parse path\to\app.apk

# Parse AAB
cargo run -- parse path\to\app.aab

# Compact single-line JSON
cargo run -- parse path\to\app.apk --compact

# Human-readable text, using frontend/templates/copy-text.template.txt by default
cargo run -- parse path\to\app.apk --text

# Use a custom text template
cargo run -- parse path\to\app.apk --text --template .\my-template.txt

# Disable stderr progress output for machine consumers
cargo run -- parse path\to\app.apk --quiet

# Export resolved icons
cargo run -- parse path\to\app.apk --export-icon .\icons

# Batch parse a directory recursively
cargo run -- parse .\samples --recursive --out result.json

# Check runtime and Android tool discovery
cargo run -- doctor
cargo run -- doctor --compact
```

CLI progress is written to stderr, while final JSON/text output remains on stdout. Use `--quiet` to disable progress output.

Build the CLI binary:

```powershell
cd backend
cargo build --release
```

The generated binary is named `apkinfoquick`.

### Recommended CLI Release Layout

For standalone CLI distribution, keep this structure:

```text
release/
|-- apkinfoquick.exe
`-- tools/
    `-- android/
        |-- aapt.exe
        `-- bundletool.jar
```

AAB parsing requires Java. Even if `bundletool.jar` is bundled, the user's system must provide `java` or `JAVA_HOME`.

### JSON Contract

All parse results use the same outer envelope:

```json
{
  "success": true,
  "data": {
    "packageName": "com.example.app",
    "appName": "Example",
    "iconUrl": "file:///...",
    "minSdkVersion": 23,
    "targetSdkVersion": 35,
    "compileSdkVersion": 35,
    "versionCode": 1,
    "versionName": "1.0.0",
    "permissions": [],
    "signers": [],
    "abis": [],
    "channel": "unknown"
  },
  "errorCode": null,
  "errorMessage": null,
  "warnings": []
}
```

Default behavior:

- `versionName`: `null` when missing.
- `compileSdkVersion`: `null` when missing.
- `channel`: `"unknown"` when unresolved.
- `permissions`, `abis`, and `signers`: `[]` when empty.

### Doctor Output

`doctor` checks the runtime environment:

- `apkReady`: whether APK parsing is available. It is usually `true` because the Rust fallback can run even without `aapt`.
- `aabReady`: whether AAB parsing is available; both `bundletool.jar` and Java are required.
- `warnings`: degraded or missing environment hints.
- `aapt`, `bundletool`, `java`, `toolsDir`: discovered paths.

### Android Tool Lookup

`aapt` lookup order:

1. Bundled Tauri resource: `tools/android/aapt.exe`.
2. Workspace path: `tools/android/aapt.exe`.
3. `APK_INFO_AAPT` environment variable.
4. `PATH`.

`bundletool` lookup order:

1. Bundled/workspace path: `tools/android/bundletool.jar`.
2. `APK_INFO_BUNDLETOOL` environment variable.

See [tools/android/README.md](tools/android/README.md) for details.

### Development

Install frontend dependencies:

```powershell
cd frontend
npm install
```

Run frontend tests:

```powershell
cd frontend
npm run test -- --run
```

Run frontend build:

```powershell
cd frontend
npm run build
```

Run backend tests:

```powershell
cd backend
cargo test
```

### Status

Current focus:

- APK/AAB parsing and display.
- Dense GUI workflow.
- CLI automation and batch processing.

Potential future improvements:

- Native AAB parsing to reduce dependency on bundletool and Java.
- Stronger signature verification.
- More export formats.

### License

License is not declared yet. Add a license before publishing this repository.

Bundled third-party Android tools may have their own licenses and redistribution requirements. Please verify them before publishing binary releases.
