# ApkInfoQuick Frontend (React + Tauri)

## Available Commands

```powershell
npm.cmd install
npm.cmd run doctor
npm.cmd run dev
npm.cmd run build
npm.cmd run test
npm.cmd run tauri:dev
```

## Current Integration

- Frontend calls Tauri command: `parse_apk`
- Tauri command maps to backend parser: `apk_info_backend::parser::parse_apk_tauri`
- UI consumes unified envelope fields:
  - `success`
  - `data`
  - `errorCode`
  - `errorMessage`
  - `warnings`

## Troubleshooting

### `cargo` not found

Install Rust toolchain:

```powershell
winget install Rustlang.Rustup
```

Restart terminal after installation, then run:

```powershell
npm.cmd run doctor
npm.cmd run tauri:dev
```

### `npm.ps1` execution policy blocked

Use `npm.cmd` instead of `npm` in PowerShell.

