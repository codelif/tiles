// Module that handles CLI commands

use anyhow::Result;
use owo_colors::OwoColorize;
use tiles::runtime::Runtime;
use tiles::utils::accounts::{
    create_root_account, get_root_user_details, save_root_account, set_nickname,
};
use tiles::utils::config::{get_or_create_config, set_memory_path};
use tiles::{core::health, runtime::RunArgs};

pub use tilekit::optimize::optimize;

use crate::{AccountArgs, AccountCommands};

pub async fn run(runtime: &Runtime, run_args: RunArgs) {
    let _ = runtime.run(run_args).await;
}

pub fn set_memory(path: &str) {
    match set_memory_path(path) {
        Ok(msg) => {
            println!("{}", msg.green());
        }
        Err(err) => {
            let error_msg = format!("Error setting memory path due to {:?}", err);
            println!("{}", error_msg.red());
        }
    }
}
pub async fn check_health() {
    health::check_health().await;
}

pub async fn start_server(runtime: &Runtime) {
    let _ = runtime.start_server_daemon().await;
}

pub async fn stop_server(runtime: &Runtime) {
    let _ = runtime.stop_server_daemon().await;
}

//TODO: add docs
pub fn run_account_commands(account_args: AccountArgs) -> Result<()> {
    let config = get_or_create_config()?;
    let root_user_details = get_root_user_details(&config)?;
    match account_args.command {
        Some(AccountCommands::Create { nickname }) => {
            //TODO: could show a message if did is already there
            let root_user_config = create_root_account(&config, nickname)?;
            let id = root_user_config.get("id").unwrap().as_str().unwrap();
            save_root_account(config, &root_user_config)?;
            //TODO: color it...
            let _ = println!("Root account has been created with id: {}", id).green();
        }
        Some(AccountCommands::SetNickname { nickname }) => {
            //FIXME: redundant code
            if root_user_details.get("id").unwrap().is_empty() {
                println!(
                    "Root account not created yet, use {}",
                    format!("tiles account create").yellow()
                );
            } else {
                match set_nickname(&config, nickname) {
                    Ok(root_user_config) => {
                        let id = root_user_config.get("id").unwrap().as_str().unwrap();
                        let nickname = root_user_config.get("nickname").unwrap().as_str().unwrap();
                        save_root_account(config, &root_user_config)?;
                        println!("Nickname {} has been set for ID: {}", nickname, id)
                    }
                    Err(err) => {
                        println!("Failed to set nickname due to {}", err)
                    }
                }
            }
        }
        _ => {
            if root_user_details.get("id").unwrap().is_empty() {
                println!(
                    "Root account not created yet, use {}",
                    format!("tiles account create").yellow()
                );
            } else {
                for elem in root_user_details {
                    println!("{}: {}", elem.0, elem.1)
                }
            }
        }
    }

    Ok(())
}
