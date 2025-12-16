/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

mod devices;
mod send_tab;

use std::fs;

use clap::{Parser, Subcommand, ValueEnum};
use cli_support::{fxa_creds, init_logging_with};
use error_support::{debug, error};
use fxa_client::{FirefoxAccount, FxaConfig, FxaServer};

static CREDENTIALS_FILENAME: &str = "credentials.json";
static CLIENT_ID: &str = "a2270f727f45f648";
static REDIRECT_URI: &str = "https://accounts.firefox.com/oauth/success/a2270f727f45f648";

use anyhow::Result;

#[derive(Parser)]
#[command(about, long_about = None)]
struct Cli {
    /// The FxA server to use
    #[arg(value_enum, default_value_t = Server::Release)]
    server: Server,

    /// Request a session scope
    #[clap(long, short, action)]
    session_scope: bool,

    /// Request a profile scope
    #[clap(long, short, action)]
    profile_scope: bool,

    /// Set the logging level to DEBUG
    #[clap(long, short, action)]
    debug: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Server {
    /// Official server
    Release,
    /// China server
    China,
    /// stable dev sever
    Stable,
    /// staging dev sever
    Stage,
    /// local dev sever
    LocalDev,
}

#[derive(Subcommand)]
enum Command {
    // Get a list of devices
    Devices(devices::DeviceArgs),
    // Send a tab
    SendTab(send_tab::SendTabArgs),
    // Load the account, check scopes and return
    Load,
    // Disconnect the example client
    Disconnect,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    nss::ensure_initialized();
    viaduct_hyper::init_backend_hyper()?;
    if cli.debug {
        init_logging_with(
            "examples_fxa_client=debug,fxa_client=debug,cli_support::fxa_creds=debug",
        );
    } else {
        init_logging_with("examples_fxa_client=info,fxa_client=info,cli_support::fxa_creds=info");
    }

    let mut scopes = vec![fxa_creds::SYNC_SCOPE];

    if cli.session_scope {
        scopes.push(fxa_creds::SESSION_SCOPE)
    }

    if cli.profile_scope {
        scopes.push(fxa_creds::PROFILE_SCOPE);
    }

    println!();
    let account = load_account(&cli, &scopes)?;
    for scope in scopes {
        match account.get_access_token(scope, None) {
            Ok(_) => debug!("Successfully fetched access token for {scope}"),
            Err(e) => {
                error!("Failed to load access token for {scope}");
                return Err(e.into());
            }
        }
    }
    match cli.command {
        Command::Devices(args) => devices::run(&account, args),
        Command::SendTab(args) => send_tab::run(&account, args),
        Command::Load => Ok(()),
        Command::Disconnect => {
            account.disconnect();
            Ok(())
        }
    }?;

    Ok(())
}

fn load_account(cli: &Cli, scopes: &[&str]) -> Result<FirefoxAccount> {
    let config = FxaConfig {
        server: match cli.server {
            Server::Release => FxaServer::Release,
            Server::Stable => FxaServer::Stable,
            Server::Stage => FxaServer::Stage,
            Server::China => FxaServer::China,
            Server::LocalDev => FxaServer::LocalDev,
        },
        redirect_uri: REDIRECT_URI.into(),
        client_id: CLIENT_ID.into(),
        token_server_url_override: None,
    };
    fxa_creds::get_cli_fxa(config, &credentials_path(), scopes).map(|cli| cli.account)
}

pub fn persist_fxa_state(acct: &FirefoxAccount) -> Result<()> {
    let json = acct.to_json().unwrap();
    Ok(fs::write(credentials_path(), json)?)
}

fn credentials_path() -> String {
    cli_support::cli_data_path(CREDENTIALS_FILENAME)
}
