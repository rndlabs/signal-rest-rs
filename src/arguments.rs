use clap::{Parser, Subcommand};
use std::path::PathBuf;
use url::Url;

use presage::{
    prelude::SignalServers,
    prelude::phonenumber::PhoneNumber,
};

use crate::logging::LoggingArguments;

#[derive(Parser)]
#[clap(about = "A Rest API to Signal relayer")]
pub struct Args {
    #[clap(flatten)]
    pub logging: LoggingArguments,

    #[clap(long = "db-path", short = 'd', group = "store")]
    pub db_path: Option<PathBuf>,

    #[clap(
        help = "passphrase to encrypt the local storage",
        long = "passphrase",
        short = 'p',
        group = "store"
    )]
    pub passphrase: Option<String>,

    #[clap(subcommand)]
    pub subcommand: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    #[clap(about = "Start the relayer")]
    Start,
    #[clap(about = "Register using a phone number")]
    Register {
        #[clap(long = "servers", short = 's', default_value = "staging")]
        servers: SignalServers,
        #[clap(long, help = "Phone Number to register with in E.164 format")]
        phone_number: PhoneNumber,
        #[clap(long)]
        use_voice_call: bool,
        #[clap(
            long = "captcha",
            help = "Captcha obtained from https://signalcaptchas.org/registration/generate.html"
        )]
        captcha: Url,
        #[clap(long, help = "Force to register again if already registered")]
        force: bool,
    },
}