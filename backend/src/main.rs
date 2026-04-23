use clap::Parser;

use apk_info_backend::cli::{CliArgs, Commands};
use apk_info_backend::parser::parse_apk_to_envelope;

fn main() {
    let args = CliArgs::parse();

    match args.command {
        Commands::Parse {
            file,
            pretty,
            compact,
        } => {
            let envelope = parse_apk_to_envelope(std::path::Path::new(&file));
            let use_compact = compact && !pretty;

            let rendered = if use_compact {
                serde_json::to_string(&envelope)
            } else {
                serde_json::to_string_pretty(&envelope)
            };

            match rendered {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    let fallback = serde_json::json!({
                        "success": false,
                        "data": {
                            "packageName": "unknown",
                            "appName": "Unknown",
                            "iconUrl": "",
                            "minSdkVersion": 1,
                            "targetSdkVersion": 1,
                            "compileSdkVersion": null,
                            "versionCode": 1,
                            "versionName": null,
                            "permissions": [],
                            "signers": [],
                            "abis": [],
                            "channel": "unknown"
                        },
                        "errorCode": "JSON_SERIALIZE_FAILED",
                        "errorMessage": err.to_string(),
                        "warnings": []
                    });
                    println!("{}", fallback);
                    std::process::exit(1);
                }
            }

            if !envelope.success {
                std::process::exit(2);
            }
        }
    }
}
