# Android tools

This directory contains the Android helper tools bundled with ApkInfoQuick.

The current parser uses `aapt.exe` as the primary APK metadata parser. Other
tools are included to keep parity with the original APK-Info release toolchain
and to support future enhancements:

- `aapt.exe`: `badging`, `resources`, and `xmltree` parsing.
- `unzip.exe`: APK entry extraction fallback.
- `apksigner.jar`: signature verification fallback.
- `dwebp.exe`: WebP conversion fallback.
- `adb.exe` and Android DLLs: optional device-side workflows.
- `curl.exe` and CA bundle: optional network lookup workflows.

Runtime lookup order for `aapt`:

1. Bundled Tauri resource: `tools/android/aapt.exe`.
2. Workspace/tool directory: `tools/android/aapt.exe`.
3. `APK_INFO_AAPT` environment variable.
4. `PATH`.

When `aapt` is not available, the app automatically falls back to the built-in
Rust parser and adds `AAPT_NOT_FOUND_FALLBACK_USED` to diagnostics.
