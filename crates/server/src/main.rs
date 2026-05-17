#![deny(
    clippy::panic,
    clippy::todo,
    clippy::unreachable,
    clippy::unimplemented,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic_in_result_fn,
    clippy::exit
)]
#![cfg_attr(not(test), deny(clippy::assertions_on_constants))]
#![deny(clippy::pedantic)]

mod app;
mod auth;
mod config;
mod deepgram;
mod handlers;
mod media_session;
mod metrics;
mod state;
mod telnyx_api;
mod utils;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}
