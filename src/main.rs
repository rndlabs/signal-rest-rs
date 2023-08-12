use clap::Parser;
use directories::ProjectDirs;
use env_logger::Env;
use crate::arguments::Args;
use signal_service::SignalServiceWrapper;
use tokio::sync::mpsc;

pub mod arguments;
pub mod service;
pub mod relayer;
pub mod signal_service;

#[tokio::main]
async fn main() {
    env_logger::from_env(
        Env::default().default_filter_or(format!("{}=warn", env!("CARGO_PKG_NAME"))),
    )
    .init();

    let args = Args::parse();
    let db_path = args.db_path.unwrap_or_else(|| {
        ProjectDirs::from("org", "whisperfish", "presage")
            .unwrap()
            .config_dir()
            .into()
    });

    // Create the channel
    let (tx, mut rx) = mpsc::unbounded_channel::<(String, String)>();

    tokio::task::spawn(service::start(tx));

    let signal_service = SignalServiceWrapper::new(db_path, args.passphrase, rx);
    signal_service.run().await;
}
