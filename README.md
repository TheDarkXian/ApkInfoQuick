# ApkInfoQuick

[中文](#中文) | [English](#english)

## 中文

ApkInfoQuick 是一个基于 React、Tauri 和 Rust 开发的桌面 APK 信息查看与解析工具。

它的目标是提供一个更现代、更适合脚本自动化的 APK 信息查看方案。你可以把 APK 拖进桌面应用，快速查看包名、应用名、版本、SDK、权限、ABI、渠道、签名、图标来源和诊断信息；也可以使用 CLI 做批量解析和流水线集成。

### 功能特性

- 基于 React、Material UI 和 Tauri 的桌面 GUI。
- 支持拖拽 APK 到多标签工作区。
- 支持多文件顺序解析，默认最多 10 个标签。
- AAB 当前作为占位标签，后续计划支持真实解析。
- 高密度信息布局，适合日常 APK 检查。
- 应用图标预览与导出。
- 当前标签内容可复制为纯文本或 JSON。
- 提供 CLI，适合脚本和批处理。
- GUI 和 CLI 使用统一的 `Envelope + data` JSON 契约。
- 混合解析管线：
  - 优先使用 `aapt dump badging/resources/xmltree`。
  - Rust 兜底解析 manifest、resources.arsc、icon、channel、ABI 和签名信息。
- 禁止浏览器原生 `alert`、`confirm`、`prompt`；交互反馈使用应用内 toast 和内联状态。

### 当前可解析信息

ApkInfoQuick 当前展示和导出的信息包括：

- 包名
- 应用名
- 应用图标与图标来源
- versionCode 和 versionName
- minSdkVersion、targetSdkVersion、compileSdkVersion
- 权限列表
- ABI 列表
- 渠道信息
- 签名信息，尽力解析
- warnings 和诊断信息
- 解析错误与重试入口

### 项目结构

```text
.
|-- backend/              Rust 解析引擎和 CLI
|-- frontend/             React UI 和 Tauri 桌面壳
|-- tools/android/        内置 Android 辅助工具，包括 aapt.exe
|-- AI_PROJECT_CONTEXT.md AI/代码接手上下文
`-- README.md
```

### GUI 使用

开发模式运行桌面应用：

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

Tauri 打包时会携带 `tools/android/*`，所以普通用户不需要自己配置 `aapt.exe`。

### CLI 使用

CLI 位于 Rust 后端，并且复用 GUI 的同一套解析引擎。

```powershell
cd backend
cargo run -- parse path\to\app.apk
```

常用命令：

```powershell
# 默认输出格式化 JSON Envelope
cargo run -- parse path\to\app.apk

# 输出单行紧凑 JSON
cargo run -- parse path\to\app.apk --compact

# 输出可读文本
cargo run -- parse path\to\app.apk --text

# 解析并导出图标
cargo run -- parse path\to\app.apk --export-icon .\icons

# 递归批量解析目录
cargo run -- parse .\samples --recursive --out result.json

# 检查运行环境和 Android 工具发现情况
cargo run -- doctor
```

构建 CLI：

```powershell
cd backend
cargo build --release
```

生成的二进制名称为 `apkinfoquick`。

### JSON 契约

所有解析结果都使用相同的外层 Envelope：

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

默认值：

- `versionName`：缺失时为 `null`。
- `compileSdkVersion`：缺失时为 `null`。
- `channel`：未解析到时为 `"unknown"`。
- `permissions`、`abis`、`signers`：为空时为 `[]`。

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

前端环境检查：

```powershell
cd frontend
npm run doctor
```

### Android 工具

ApkInfoQuick 在 `tools/android/` 下内置 Android 辅助工具。

`aapt` 查找顺序：

1. Tauri 内置资源：`tools/android/aapt.exe`。
2. 工作区路径：`tools/android/aapt.exe`。
3. `APK_INFO_AAPT` 环境变量。
4. `PATH`。

如果找不到 `aapt`，解析器会自动回退到内置 Rust 解析器，并在诊断信息中加入 `AAPT_NOT_FOUND_FALLBACK_USED`。

更多说明见 [tools/android/README.md](tools/android/README.md)。

### 当前状态

当前重点：

- APK 解析与展示。
- GUI 使用体验优化。
- CLI 自动化支持。

计划：

- AAB 真实解析支持。
- 更完整的签名验证。
- 更多导出格式。

### 许可证

本仓库暂未声明许可证。正式发布或分发前请补充 LICENSE。

内置第三方 Android 工具可能有各自的许可证要求，发布二进制包前请确认再分发条款。

## English

ApkInfoQuick is a fast desktop APK metadata viewer and parser built with React, Tauri, and Rust.

The goal is to provide a modern, script-friendly alternative to older APK info tools. Drop an APK into the desktop app to inspect package metadata, app name, versions, SDK levels, permissions, ABI, channel, signatures, icon source, and diagnostics. Use the CLI for automation and batch workflows.

### Features

- Desktop GUI based on React, Material UI, and Tauri.
- Drag and drop APK files into a multi-tab workspace.
- Sequential multi-file parsing, with a default limit of 10 tabs.
- AAB placeholder tabs for future support.
- Dense metadata layout for daily APK inspection.
- App icon preview and export.
- Current-tab copy as plain text or JSON.
- CLI parser for scripts and batch workflows.
- Unified `Envelope + data` JSON contract across GUI and CLI.
- Hybrid parser pipeline:
  - `aapt dump badging/resources/xmltree` as the primary parser.
  - Rust fallback for manifest, resources.arsc, icon, channel, ABI, and best-effort signature parsing.
- No browser-native `alert`, `confirm`, or `prompt`; UI feedback uses in-app toast and inline states.

### Parsed Data

ApkInfoQuick currently displays and exports:

- Package name
- App name
- App icon and icon source
- Version code and version name
- minSdkVersion, targetSdkVersion, and compileSdkVersion
- Permissions
- ABI list
- Channel
- Signer information, best-effort
- Warnings and diagnostics
- Parse errors with retry support

### Project Structure

```text
.
|-- backend/              Rust parser engine and CLI
|-- frontend/             React UI and Tauri desktop shell
|-- tools/android/        Bundled Android helper tools, including aapt.exe
|-- AI_PROJECT_CONTEXT.md Notes for AI/codebase handoff
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

The Tauri bundle includes `tools/android/*`, so end users do not need to configure `aapt.exe` manually.

### CLI Usage

The CLI lives in the Rust backend and uses the same parser engine as the GUI.

```powershell
cd backend
cargo run -- parse path\to\app.apk
```

Common commands:

```powershell
# Pretty JSON Envelope, default
cargo run -- parse path\to\app.apk

# Compact single-line JSON
cargo run -- parse path\to\app.apk --compact

# Human-readable text output
cargo run -- parse path\to\app.apk --text

# Export resolved icon
cargo run -- parse path\to\app.apk --export-icon .\icons

# Batch parse a directory recursively
cargo run -- parse .\samples --recursive --out result.json

# Check runtime and Android tool discovery
cargo run -- doctor
```

Build the CLI binary:

```powershell
cd backend
cargo build --release
```

The generated binary is named `apkinfoquick`.

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

Optional frontend health check:

```powershell
cd frontend
npm run doctor
```

### Android Tools

ApkInfoQuick bundles Android helper tools under `tools/android/`.

Runtime `aapt` lookup order:

1. Bundled Tauri resource: `tools/android/aapt.exe`.
2. Workspace path: `tools/android/aapt.exe`.
3. `APK_INFO_AAPT` environment variable.
4. `PATH`.

If `aapt` is unavailable, the parser falls back to the built-in Rust parser and emits `AAPT_NOT_FOUND_FALLBACK_USED` in diagnostics.

See [tools/android/README.md](tools/android/README.md) for details.

### Status

Current focus:

- APK parsing and display.
- GUI workflow optimization.
- CLI automation support.

Planned:

- Real AAB parsing support.
- Stronger signature verification.
- More export formats.

### License

License is not declared yet. Add a license before publishing or distributing this repository.

Bundled third-party Android tools may have their own licenses. Please verify redistribution requirements before publishing binary releases.
