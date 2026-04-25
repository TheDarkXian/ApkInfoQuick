# ApkInfoQuick

ApkInfoQuick is a fast desktop APK metadata viewer and parser built with React,
Tauri, and Rust.

The goal is to provide a modern, script-friendly alternative to older APK info
tools. Drop an APK into the desktop app to inspect package metadata, app name,
versions, SDK levels, permissions, ABI, channel, signatures, icon source, and
diagnostics. Use the CLI for automation and batch workflows.

## Features

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
  - Rust fallback for manifest, resources.arsc, icon, channel, ABI, and
    best-effort signature parsing.
- No browser-native `alert`, `confirm`, or `prompt`; UI feedback uses in-app
  toast and inline states.

## Parsed Data

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

## Project Structure

```text
.
|-- backend/              Rust parser engine and CLI
|-- frontend/             React UI and Tauri desktop shell
|-- tools/android/        Bundled Android helper tools, including aapt.exe
|-- AI_PROJECT_CONTEXT.md Notes for AI/codebase handoff
`-- README.md
```

## GUI Usage

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

The Tauri bundle includes `tools/android/*`, so end users do not need to
configure `aapt.exe` manually.

## CLI Usage

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

## JSON Contract

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

## Development

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

## Android Tools

ApkInfoQuick bundles Android helper tools under `tools/android/`.

Runtime `aapt` lookup order:

1. Bundled Tauri resource: `tools/android/aapt.exe`.
2. Workspace path: `tools/android/aapt.exe`.
3. `APK_INFO_AAPT` environment variable.
4. `PATH`.

If `aapt` is unavailable, the parser falls back to the built-in Rust parser and
emits `AAPT_NOT_FOUND_FALLBACK_USED` in diagnostics.

See [tools/android/README.md](tools/android/README.md) for details.

## Status

Current focus:

- APK parsing and display.
- GUI workflow optimization.
- CLI automation support.

Planned:

- Real AAB parsing support.
- Stronger signature verification.
- More export formats.

## License

License is not declared yet. Add a license before publishing or distributing
this repository.

Bundled third-party Android tools may have their own licenses. Please verify
redistribution requirements before publishing binary releases.
