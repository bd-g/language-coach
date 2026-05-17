use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// Command-line options for the debug client.
#[derive(Debug, Parser)]
pub(crate) struct Cli {
    #[arg(long)]
    pub(crate) config: PathBuf,
    #[arg(long)]
    pub(crate) url: Option<String>,
    #[arg(long, value_enum, default_value_t = AuthMode::Subprotocol)]
    pub(crate) auth_mode: AuthMode,
}

/// Debug client authentication transport mode.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub(crate) enum AuthMode {
    Subprotocol,
    Authorization,
}
