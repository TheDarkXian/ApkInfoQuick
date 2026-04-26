use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApkInfoEnvelope {
    pub success: bool,
    pub data: ApkInfoData,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApkInfoData {
    pub package_name: String,
    pub app_name: String,
    pub icon_url: String,
    pub min_sdk_version: i32,
    pub target_sdk_version: i32,
    pub compile_sdk_version: Option<i32>,
    pub version_code: i64,
    pub version_name: Option<String>,
    pub permissions: Vec<String>,
    pub signers: Vec<SignerInfo>,
    pub abis: Vec<String>,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerInfo {
    pub scheme: String,
    pub cert_sha256: String,
    pub issuer: String,
    pub subject: String,
    pub valid_from: String,
    pub valid_to: String,
}

impl ApkInfoData {
    pub fn placeholder() -> Self {
        Self {
            package_name: "unknown".to_string(),
            app_name: "Unknown".to_string(),
            icon_url: String::new(),
            min_sdk_version: 1,
            target_sdk_version: 1,
            compile_sdk_version: None,
            version_code: 1,
            version_name: None,
            permissions: Vec::new(),
            signers: Vec::new(),
            abis: Vec::new(),
            channel: "unknown".to_string(),
        }
    }
}

impl ApkInfoEnvelope {
    pub fn ok(data: ApkInfoData, warnings: Vec<String>) -> Self {
        Self {
            success: true,
            data,
            error_code: None,
            error_message: None,
            warnings,
        }
    }

    pub fn err(code: &str, message: String, data: ApkInfoData, warnings: Vec<String>) -> Self {
        Self {
            success: false,
            data,
            error_code: Some(code.to_string()),
            error_message: Some(message),
            warnings,
        }
    }
}
