use arguments::Cmd;
use clap::Parser;
use directories::ProjectDirs;
use presage::{Manager, RegistrationOptions, Store};
use presage_store_sled::{SledStore, MigrationConflictStrategy};
use crate::arguments::Args;
use signal_service::SignalServiceWrapper;
use tokio::sync::mpsc;
use tokio::io::{self, AsyncBufReadExt, BufReader};

pub mod arguments;
pub mod service;
pub mod relayer;
pub mod signal_service;
pub mod logging;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let config_store = SledStore::open_with_passphrase(
        db_path.clone(),
        args.passphrase,
        MigrationConflictStrategy::Raise,
    ).expect("failed to open config database");

    run(args.subcommand, config_store).await
}

async fn run<C: Store + 'static>(subcommand: Cmd, config_store: C) -> Result<(), Box<dyn std::error::Error>> {
    match subcommand {
        Cmd::Register {
            servers,
            phone_number,
            use_voice_call,
            captcha,
            force,
        } => {
            let manager = Manager::register(
                config_store,
                RegistrationOptions {
                    signal_servers: servers,
                    phone_number,
                    use_voice_call,
                    captcha: Some(captcha.host_str().unwrap()),
                    force,
                },
            )
            .await?;

            // ask for confirmation code here
            let stdin = io::stdin();
            let reader = BufReader::new(stdin);
            if let Some(confirmation_code) = reader.lines().next_line().await? {
                manager.confirm_verification_code(confirmation_code).await?;
            } else {
                return Err("Failed to read confirmation code from stdin".into());
            }
        },
        Cmd::Start => {
            // Create the channel
            let (tx, rx) = mpsc::unbounded_channel::<(String, String)>();
        
            tokio::task::spawn(service::start(tx));
        
            let signal_service = SignalServiceWrapper::new(rx, config_store.clone());
            signal_service.run().await;
        }
    }
    
    Ok(())
}