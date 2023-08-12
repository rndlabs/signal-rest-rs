use clap::{ArgGroup, Parser, Subcommand};
use std::path::PathBuf;
use url::Url;
use anyhow::anyhow;

use presage::{
    prelude::SignalServers,
    prelude::{phonenumber::PhoneNumber, Uuid},
    GroupMasterKeyBytes,
};
use presage::libsignal_service::prelude::ProfileKey;

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
    Relayer,
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
    #[clap(about = "Unregister from Signal")]
    Unregister,
    #[clap(
        about = "Generate a QR code to scan with Signal for iOS or Android to link this client as secondary device"
    )]
    LinkDevice {
        /// Possible values: staging, production
        #[clap(long, short = 's', default_value = "production")]
        servers: SignalServers,
        #[clap(
            long,
            short = 'n',
            help = "Name of the device to register in the primary client"
        )]
        device_name: String,
    },
    #[clap(about = "Get information on the registered user")]
    Whoami,
    #[clap(about = "Retrieve the user profile")]
    RetrieveProfile {
        /// Id of the user to retrieve the profile. When omitted, retrieves the registered user
        /// profile.
        #[clap(long)]
        uuid: Uuid,
        /// Base64-encoded profile key of user to be able to access their profile
        #[clap(long, value_parser = parse_base64_profile_key)]
        profile_key: Option<ProfileKey>,
    },
    #[clap(about = "Set a name, status and avatar")]
    UpdateProfile,
    #[clap(about = "Check if a user is registered on Signal")]
    GetUserStatus,
    #[clap(about = "Block contacts or groups")]
    Block,
    #[clap(about = "Unblock contacts or groups")]
    Unblock,
    #[clap(about = "Update the details of a contact")]
    UpdateContact,
    #[clap(about = "Receive all pending messages and saves them to disk")]
    Receive {
        #[clap(long = "notifications", short = 'n')]
        notifications: bool,
    },
    #[clap(about = "List groups")]
    ListGroups,
    #[clap(about = "List contacts")]
    ListContacts,
    #[clap(
        about = "List messages",
        group(
            ArgGroup::new("list-messages")
                .required(true)
                .args(&["recipient_uuid", "group_master_key"])
        )
    )]
    ListMessages {
        #[clap(long, short = 'u', help = "recipient UUID")]
        recipient_uuid: Option<Uuid>,
        #[clap(
            long,
            short = 'k',
            help = "Master Key of the V2 group (hex string)",
            value_parser = parse_group_master_key,
        )]
        group_master_key: Option<GroupMasterKeyBytes>,
        #[clap(long, help = "start from the following date (UNIX timestamp)")]
        from: Option<u64>,
    },
    #[clap(about = "Get a single contact by UUID")]
    GetContact { uuid: Uuid },
    #[clap(about = "Find a contact in the embedded DB")]
    FindContact {
        #[clap(long, short = 'u', help = "contact UUID")]
        uuid: Option<Uuid>,
        #[clap(long, short = 'p', help = "contact phone number")]
        phone_number: Option<PhoneNumber>,
        #[clap(long, short = 'n', help = "contact name")]
        name: Option<String>,
    },
    #[clap(about = "Send a message")]
    Send {
        #[clap(long, short = 'u', help = "uuid of the recipient")]
        uuid: Uuid,
        #[clap(long, short = 'm', help = "Contents of the message to send")]
        message: String,
    },
    #[clap(about = "Send a message to group")]
    SendToGroup {
        #[clap(long, short = 'm', help = "Contents of the message to send")]
        message: String,
        #[clap(long, short = 'k', help = "Master Key of the V2 group (hex string)", value_parser = parse_group_master_key)]
        master_key: GroupMasterKeyBytes,
    },
    #[cfg(feature = "quirks")]
    RequestSyncContacts,
}

fn parse_group_master_key(value: &str) -> anyhow::Result<GroupMasterKeyBytes> {
    let master_key_bytes = hex::decode(value)?;
    master_key_bytes
        .try_into()
        .map_err(|_| anyhow::format_err!("master key should be 32 bytes long"))
}

fn parse_base64_profile_key(s: &str) -> anyhow::Result<ProfileKey> {
    let bytes = base64::decode(s)?
        .try_into()
        .map_err(|_| anyhow!("profile key of invalid length"))?;
    Ok(ProfileKey::create(bytes))
}
