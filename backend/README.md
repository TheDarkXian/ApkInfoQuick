# Backend (Rust)

## Current scope
- Unified `Envelope + data` contract for both CLI and Tauri service function.
- Parses APK zip, reads `AndroidManifest.xml` (text XML and partial binary AXML support).
- Extracts: package/app/version/sdk/permissions, ABI list, channel, icon URL, and partial signer info.
- Applies required defaults:
  - `versionName`: `null` when missing
  - `compileSdkVersion`: `null` when missing
  - `channel`: `"unknown"` when unresolved
  - `permissions` / `abis` / `signers`: `[]` when empty

## CLI
```powershell
cd backend
cargo run -- parse path\to\app.apk
cargo run -- parse path\to\app.apk --compact
cargo run -- parse path\to\app.apk --pretty
cargo run -- parse path\to\app.apk --text
cargo run -- parse path\to\app.apk --export-icon .\icons
cargo run -- parse .\samples --recursive --out result.json
cargo run -- doctor
```

- `--pretty`: pretty JSON (default behavior)
- `--compact`: single-line JSON
- `--text`: human-readable text output
- `--recursive`: recursively scan directories for `.apk` files
- `--out <file>`: write output to a file instead of stdout
- `--export-icon <dir>`: copy the resolved icon file into the chosen directory
- `doctor`: reports the resolved `aapt` path and bundled tools directory
- Single-file JSON output remains the raw `Envelope`; multi-file output is an array of `{ path, envelope, iconExportedTo }`

## Channel priority
1. Manifest `meta-data` (`UMENG_CHANNEL`, `CHANNEL`, ...)
2. `META-INF/channel_*`
3. APK filename token
4. Fallback to `unknown` + warning `CHANNEL_NOT_FOUND`

## Notes
- Signature parsing is best-effort: v1 cert container files are hashed and mapped to `signers[]`.
- If signature hints are present but full certificate metadata is unavailable, warning `SIGNATURE_PARTIAL` is emitted.
- Unknown signer validity period is returned as empty string instead of fabricated timestamps.
- App label supports `@string/...` lookup from `res/values*/strings.xml` when available.
- Icon extraction is best-effort from manifest resource name and fallback search in `res/mipmap*` and `res/drawable*`.
