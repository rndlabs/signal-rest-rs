use clap::Parser;
use directories::ProjectDirs;
use crate::arguments::Args;
use signal_service::SignalServiceWrapper;
use tokio::sync::mpsc;

pub mod arguments;
pub mod service;
pub mod relayer;
pub mod signal_service;
pub mod logging;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    logging::initialize(
        args.logging.log_filter.as_str(),
        args.logging.log_stderr_threshold,
    );

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
