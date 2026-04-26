use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("INPUT_NOT_FOUND: file does not exist")]
    InputNotFound,
    #[error("INPUT_NOT_APK_OR_AAB: file extension must be .apk or .aab")]
    InputNotApk,
    #[error("INPUT_NOT_FILE: path is not a regular file")]
    InputNotFile,
    #[error("APK_OPEN_FAILED: unable to open apk archive")]
    ApkOpenFailed,
    #[error("AAB_CONVERT_FAILED: unable to convert aab to universal apk")]
    AabConvertFailed,
    #[error("APK_ENTRY_READ_FAILED: unable to read apk entry")]
    ApkEntryReadFailed,
    #[error("MANIFEST_NOT_FOUND: AndroidManifest.xml is missing")]
    ManifestNotFound,
    #[error("MANIFEST_PARSE_FAILED: unable to parse AndroidManifest.xml")]
    ManifestParseFailed,
    #[error("PARSE_LIMIT_EXCEEDED: parser limit exceeded")]
    ParseLimitExceeded,
}

impl BackendError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InputNotFound => "INPUT_NOT_FOUND",
            Self::InputNotApk => "INPUT_NOT_APK",
            Self::InputNotFile => "INPUT_NOT_FILE",
            Self::ApkOpenFailed => "APK_OPEN_FAILED",
            Self::AabConvertFailed => "AAB_CONVERT_FAILED",
            Self::ApkEntryReadFailed => "APK_ENTRY_READ_FAILED",
            Self::ManifestNotFound => "MANIFEST_NOT_FOUND",
            Self::ManifestParseFailed => "MANIFEST_PARSE_FAILED",
            Self::ParseLimitExceeded => "PARSE_LIMIT_EXCEEDED",
        }
    }
}
