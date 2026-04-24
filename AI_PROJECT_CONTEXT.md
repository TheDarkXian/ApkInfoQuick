# ApkInfoQuick 项目现状（供 AI 接手）

更新时间：2026-04-23

## 1. 项目目标
- 一个 APK 信息解析桌面工具。
- 技术栈：React + Tauri + Rust。
- 核心链路：拖拽/选择文件 -> 后端解析 -> 前端按统一 Envelope + data 契约展示。

## 2. 当前结构（关键目录）
- `backend/`：Rust 解析引擎与 CLI。
- `frontend/`：React 前端与 Tauri 壳。
- `需求/`：需求文档。
- `测试/`：测试清单。

## 3. 已实现能力
### 前端
- Vite + React + TypeScript + MUI。
- 已支持：
  - 多文件拖拽/文件选择（APK + AAB），标签上限 10。
  - APK 按输入顺序自动串行解析（无需手动逐个点解析）。
  - AAB 占位标签（v1.0 不做真实解析）。
  - 标签工作区操作：关闭当前、关闭其他、清空全部。
  - 标签页顶部栏显示完整路径，右侧显示 icon 与动作。
  - Envelope + data 展示、错误态与重试、warnings 展示。
  - 当前标签复制：纯文本（模板占位符）与 JSON。
  - 统一应用内 toast 反馈。

### Tauri 桥接
- 命令：`parse_apk(file_path: String) -> ApkInfoEnvelope`。
- 映射：`apk_info_backend::parser::parse_apk_tauri`。
- 命令：`pick_files() -> Vec<String>`，过滤 `apk/aab` 多选路径。

### 后端
- 统一模型：`ApkInfoEnvelope / ApkInfoData / SignerInfo`。
- CLI：`apk-info parse <file>`。
- 解析能力：Manifest（文本 XML + 部分二进制 AXML）、版本/权限/ABI/渠道/图标/签名（best-effort）。

## 4. 契约与默认值
- 外层字段：`success`、`data`、`errorCode`、`errorMessage`、`warnings`。
- `data` 关键字段：
  - `packageName`, `appName`, `iconUrl`
  - `minSdkVersion`, `targetSdkVersion`, `compileSdkVersion`
  - `versionCode`, `versionName`
  - `permissions[]`, `signers[]`, `abis[]`, `channel`
- 默认值：
  - `versionName = null`
  - `compileSdkVersion = null`
  - `channel = "unknown"`
  - `permissions/abis/signers = []`

## 5. 已验证状态
- 前端：`npm.cmd run build`、`npm.cmd run test -- --run` 通过。
- 后端：`cargo test` 通过。
- Tauri：`npm.cmd run tauri:build -- --debug` 可产出安装包。

## 6. 风险状态
- 保留风险：签名解析仍是 best-effort，不是完整证书链全量实现。
- 已缓解：前端在 `warnings` 包含 `SIGNATURE_PARTIAL` / `SIGNATURE_BLOCK_DETECTED_UNPARSED` 时显示风险提示。
- 已缓解：新增 `workspace` 纯函数与单测，修复“上限截断误伤后续合法文件”的潜在风险。
- 已缓解：新增规则测试，前端源码门禁禁止 `alert/confirm/prompt`。
- Git：项目已初始化本地仓库。

## 7. 新增产品设定（2026-04-23）
以下为用户已确认的强约束：
1. AAB 暂时仅占位标签，不做真实解析。
2. UI 继续保持卡片式，但要更紧凑。
3. 复制按钮复制“当前标签内容”。
4. 复制提供两种：纯文本、JSON。
5. 文件路径展示放在“标签页内容区顶部栏”（不是标签标题本身）。
6. 顶部栏右侧展示 icon。
6. 标签命名规则：使用文件名。
7. 标签数量上限：10。
8. 需要操作：关闭当前、关闭其他、清空全部。
9. 多文件拖入时按顺序自动解析，不需要手动触发。
10. 解析失败也要保留该标签与路径，并展示错误状态。
11. 超过数量上限采用 toast 提示。
12. AAB 标签文案统一为“占位”。
13. 复制文本字段顺序可配置，使用占位符模板文件（示例：`#packname#`、`#product_name#`）。
14. 项目中禁止使用浏览器原生弹窗：`alert` / `confirm` / `prompt`。
15. 所有交互反馈统一使用应用内组件（如 toast、对话框组件、内联提示）。

## 8. 对应设计文档
- `需求/多文件标签与复制设计方案.md`
