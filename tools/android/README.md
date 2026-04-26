# Android tools

This directory contains Android helper tools used by ApkInfoQuick.

- `aapt.exe`: primary APK metadata parsing through `badging`, `resources`, and `xmltree`.
- `bundletool.jar`: AAB support. ApkInfoQuick converts `.aab` files to a universal APK, then reuses the existing APK parser.

Runtime lookup:

- `aapt`: bundled `tools/android/aapt.exe`, workspace `tools/android/aapt.exe`, `APK_INFO_AAPT`, then `PATH`.
- `bundletool`: bundled/workspace `tools/android/bundletool.jar`, then `APK_INFO_BUNDLETOOL`.

Notes:

- AAB parsing requires Java because bundletool is a Java application.
- If Java or bundletool is missing, AAB parsing returns a failed envelope with diagnostics.
