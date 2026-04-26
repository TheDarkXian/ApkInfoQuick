use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "apkinfoquick")]
#[command(version)]
#[command(about = "Parse APK metadata with the same engine as ApkInfoQuick GUI")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Parse APK/AAB files and print JSON or text.
    Parse {
        /// APK/AAB file path(s), or directories when --recursive is used.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Recursively scan directories for .apk/.aab files.
        #[arg(short, long, action = ArgAction::SetTrue, default_value_t = false)]
        recursive: bool,

        /// Print text summary instead of JSON.
        #[arg(long, action = ArgAction::SetTrue, default_value_t = false)]
        text: bool,

        /// Print pretty JSON (default).
        #[arg(long, action = ArgAction::SetTrue, default_value_t = false)]
        pretty: bool,

        /// Print compact single-line JSON.
        #[arg(long, action = ArgAction::SetTrue, default_value_t = false)]
        compact: bool,

        /// Write output to a file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Export resolved icons to this directory.
        #[arg(long)]
        export_icon: Option<PathBuf>,
    },

    /// Check CLI runtime and bundled Android tools.
    Doctor {
        /// Print compact single-line JSON.
        #[arg(long, action = ArgAction::SetTrue, default_value_t = false)]
        compact: bool,
    },
}
