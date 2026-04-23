use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "apk-info")]
#[command(version)]
#[command(about = "APK metadata parser backend CLI")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Parse an APK and print Envelope JSON.
    Parse {
        /// APK file path.
        file: String,

        /// Print pretty JSON (default).
        #[arg(long, action = ArgAction::SetTrue, default_value_t = false)]
        pretty: bool,

        /// Print compact single-line JSON.
        #[arg(long, action = ArgAction::SetTrue, default_value_t = false)]
        compact: bool,
    },
}
