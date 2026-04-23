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
```

- `--pretty`: pretty JSON (default behavior)
- `--compact`: single-line JSON

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
