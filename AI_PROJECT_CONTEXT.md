# ApkInfoQuick 项目现状（供 AI 接手）

更新时间：2026-04-23

## 1. 项目目标
- 这是一个 `APK 信息解析桌面工具`。
- 技术架构：`React + Tauri + Rust`。
- 核心能力：上传/拖拽 APK，调用 Rust 解析，按统一 `Envelope + data` 契约展示结果。

## 2. 当前目录结构（关键）
- `backend/`：Rust 解析引擎 + CLI
- `frontend/`：React 前端 + Tauri 壳
- `需求/`：需求文档（`初步方案.md`）
- `测试/`：测试清单

## 3. 已实现状态
### 前端（frontend）
- 已完成 Vite + React + TypeScript + MUI 搭建。
- 已实现：
  - `.apk` 拖拽/选择
  - 调用 Tauri 命令 `parse_apk`
  - `Envelope + data` 展示
  - 错误态（`errorCode`/`errorMessage`）+ 重试
  - 图标预览与导出按钮
  - warnings 展示
- 关键文件：
  - `frontend/src/App.tsx`
  - `frontend/src/types/apk.ts`
  - `frontend/src/services/tauri.ts`

### Tauri 桥接（frontend/src-tauri）
- 已完成命令注册：
  - `parse_apk(file_path: String) -> ApkInfoEnvelope`
  - 调用后端：`apk_info_backend::parser::parse_apk_tauri`
- 关键文件：
  - `frontend/src-tauri/src/main.rs`
  - `frontend/src-tauri/tauri.conf.json`
  - `frontend/src-tauri/icons/icon.ico`

### 后端（backend）
- 已实现：
  - 统一数据模型 `ApkInfoEnvelope / ApkInfoData / SignerInfo`
  - CLI：`apk-info parse <file>`
  - APK ZIP 读取、Manifest 解析（文本 XML + 部分二进制 AXML）
  - 字段提取：包名、应用名、SDK、版本、权限、ABI、渠道、图标、签名（best-effort）
  - 统一默认值与错误输出
- 关键文件：
  - `backend/src/model.rs`
  - `backend/src/parser.rs`
  - `backend/src/main.rs`
  - `backend/src/cli.rs`

## 4. 数据契约（最重要）
- 外层统一：
  - `success: boolean`
  - `data: object`
  - `errorCode: string | null`
  - `errorMessage: string | null`
  - `warnings: string[]`
- `data` 核心字段：
  - `packageName`, `appName`, `iconUrl`
  - `minSdkVersion`, `targetSdkVersion`, `compileSdkVersion`
  - `versionCode`, `versionName`
  - `permissions[]`, `signers[]`, `abis[]`, `channel`
- 默认值策略：
  - `versionName => null`
  - `compileSdkVersion => null`
  - `channel => "unknown"`
  - `permissions/abis/signers => []`

## 5. 渠道解析优先级
1. Manifest `meta-data`（如 `UMENG_CHANNEL` / `CHANNEL`）
2. `META-INF/channel_*`
3. 文件名推断
4. 否则 `unknown` + `CHANNEL_NOT_FOUND`

## 6. 可用命令（Windows / PowerShell）
### 前端
```powershell
cd frontend
npm.cmd install
npm.cmd run doctor
npm.cmd run dev
npm.cmd run build
npm.cmd run test
npm.cmd run tauri:dev
npm.cmd run tauri:build -- --debug
```

### 后端
```powershell
cd backend
cargo test
cargo run -- parse path\to\app.apk
cargo run -- parse path\to\app.apk --compact
cargo run -- parse path\to\app.apk --pretty
```

## 7. 当前验证结果
- `frontend`：
  - `npm.cmd run test` 通过（含类型归一化与 tauri service 测试）
  - `npm.cmd run build` 通过
  - `npm.cmd run tauri:build -- --debug` 通过
- `backend`：
  - `cargo test` 通过
- 已产物：
  - `frontend/src-tauri/target/debug/bundle/msi/ApkInfoQuick_0.1.0_x64_en-US.msi`
  - `frontend/src-tauri/target/debug/bundle/nsis/ApkInfoQuick_0.1.0_x64-setup.exe`

## 8. 已知问题 / 风险
- 后端签名解析仍是 best-effort，不是完整证书链全量实现。
- 前端已增加签名风险提示（当 warnings 包含 `SIGNATURE_PARTIAL` / `SIGNATURE_BLOCK_DETECTED_UNPARSED` 时显示）。
- 项目已初始化本地 Git 仓库，`git status` 可用。

## 9. AI 接手建议（优先级）
1. 先修复前端文案乱码（统一 UTF-8 编码并替换字符串）。
2. 跑一次真机联调：`npm.cmd run tauri:dev`，用多 APK 样本回归。
3. 完善签名解析与错误码文档，减少 `SIGNATURE_PARTIAL` 场景。
4. 增加端到端测试（拖拽 -> invoke -> 展示 -> 错误重试）。
