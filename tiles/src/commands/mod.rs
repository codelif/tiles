// Module that handles CLI commands

use std::io;

use anyhow::{Result, anyhow};
use owo_colors::OwoColorize;
use tiles::runtime::Runtime;
use tiles::utils::accounts::{
    RootUser, create_root_account, get_root_user_details, save_root_account, set_nickname,
};
use tiles::utils::config::{
    ConfigProvider, DefaultProvider, get_or_create_config, set_user_data_path,
};
use tiles::{core::health, runtime::RunArgs};

pub use tilekit::optimize::optimize;
use toml::Table;

use crate::{AccountArgs, AccountCommands};

pub fn run_setup_for_ftue() -> Result<()> {
    // initializes config directory
    let config_provider = DefaultProvider;
    config_provider.get_or_create_config_dir()?;
    config_provider.get_or_create_data_dir()?;

    let root_config = get_or_create_config()?;
    let root_user_details = get_root_user_details(&root_config)?;
    if root_user_details.id.is_empty() {
        println!(
            "
              ▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓▓░             
             ▓▓                      ▓▓░░▓▒            
           ░▓▓░░░░░░░░░░     ░░░░░░░▓▓    ▓▒           
            ▓▓░░░░░░░▓▓░    ▓▓▓░░░░░▓▓   ▓▓            
             ▓▓     ░▓▒    ▓▓ ▓▒     ▒▓░▓▓             
              ▓▓▓▓▓▓▓▒    ▓▓   ▓▓▓▓▓▓▓▓▓▓              
                   ▓▓    ▓▓   ░▓░                      
                  ▓▓    ▒▓░   ▓▒                       
                 ▒▓    ░▓░   ▓▓                        
                ▒▓    ░▓▒   ▓▓                         
               ░▓░    ▓▓   ▓▓                          
              ░▓▒    ▓▓   ▒▓                           
              ▓▓▓▓▓▓▓▓   ▒▓░                           
              ░▓▒    ▓▓ ░▓░                            
                ▓▓    ▓▓▓▒                             
                 ▓▓▓▓▓▓▓▓                              
                                                       
            "
        );

        println!(
            "{}",
            "Welcome to Tiles: Your private and secure AI assistant for everyday use.\n"
                .to_string()
                .bold()
                .blue()
        );
        // FTUE
        setup_root_account(root_config.clone())?;
        setup_default_user_data_dir(&config_provider)?
    }

    Ok(())
}

fn setup_root_account(root_config: Table) -> Result<()> {
    println!("{}",
            "\nPlease set a nickname for your local Tiles account (You can change this later via `tiles account set-nickname`)\n".to_string().cyan()
        );
    let stdin = io::stdin();
    let mut input = String::new();
    stdin.read_line(&mut input)?;
    input = input.trim().to_owned();
    let root_user_config = RootUser::new(&create_root_account(&root_config, Some(input))?)?;

    save_root_account(root_config, &root_user_config.to_table())?;
    println!(
        "{}",
        format_args!(
            "\nYour Tiles local account: {} has been created with nickname {}\n",
            root_user_config.id, root_user_config.nickname
        )
        .green()
    );
    Ok(())
}

fn setup_default_user_data_dir<T: ConfigProvider>(config_provider: &T) -> Result<()> {
    // gets default data dir -> ~/.local/share/tiles/data
    // shows this is the data dir
    // asks if they want to change, if y, asks for new loc, else keep current one
    // writes the default/new path to in config.toml data->path
    //
    let user_data_dir = config_provider.get_user_data_dir()?;
    println!(
        "{}",
        format!(
            "\nYour Default user data location will be set at {:?}\n",
            user_data_dir
        )
        .yellow()
    );
    println!("\nYou can always change the location with `tiles data set-path <PATH>`\n");
    println!(
        "{}",
        "\nDo you want to add a custom user data location right now instead? [Y/N]"
            .to_string()
            .cyan()
    );
    let mut input = String::new();
    let stdin = io::stdin();
    let mut chose_yes = false;
    loop {
        input.clear();
        stdin.read_line(&mut input)?;
        input = input.trim().to_owned();
        if (input == "Y" || input == "y") || chose_yes {
            if !chose_yes {
                chose_yes = true;
                println!("Add the path for your custom user data location");
                continue;
            }
            match set_user_data_path(input.as_str()) {
                Ok(msg) => {
                    println!("{}", msg.green());
                    println!(
                        "\nYou can always change the location with `tiles data set-path <PATH>`\n"
                    );
                    break;
                }
                Err(err) => {
                    let error_msg =
                        format!("\nTry again, Error setting user data path due to {:?}", err);
                    println!("{}", error_msg.red());
                    continue;
                }
            }
        } else {
            match set_user_data_path(
                user_data_dir
                    .to_str()
                    .ok_or_else(|| anyhow!("Failed to parse user data dir"))?,
            ) {
                Ok(msg) => {
                    println!("{}", msg.green());
                    println!(
                        "\nYou can always change the location with `tiles data set-path <PATH>`\n"
                    );
                    break;
                }
                Err(err) => {
                    let error_msg = format!("Error setting user data path due to {:?}", err);
                    println!("{}", error_msg.red());
                    return Err(anyhow::anyhow!("Error setting default user data path"));
                }
            }
        }
    }
    Ok(())
}

pub async fn run(runtime: &Runtime, run_args: RunArgs) {
    let _ = runtime.run(run_args).await;
}

pub fn set_data(path: &str) {
    match set_user_data_path(path) {
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

/// Runs the account command with the args being passed.
pub fn run_account_commands(account_args: AccountArgs) -> Result<()> {
    let config = get_or_create_config()?;
    let root_user_details = get_root_user_details(&config)?;
    match account_args.command {
        Some(AccountCommands::Create { nickname }) => {
            if !root_user_details.id.is_empty() {
                println!("Local Identity exists with id: {}", root_user_details.id)
            } else {
                let root_user_config = RootUser::new(&create_root_account(&config, nickname)?)?;

                save_root_account(config, &root_user_config.to_table())?;
                println!(
                    "{}",
                    format_args!(
                        "Local Identity has been created with id: {}",
                        root_user_config.id
                    )
                )
            }
        }
        Some(AccountCommands::SetNickname { nickname }) => {
            if root_user_details.id.is_empty() {
                println!("{}", get_account_not_created_msg());
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
            if root_user_details.id.is_empty() {
                println!("{}", get_account_not_created_msg());
            } else {
                println!("{}", root_user_details);
            }
        }
    }

    Ok(())
}

fn get_account_not_created_msg() -> String {
    format!(
        "Local Identity not created yet, use {}",
        "tiles account create".yellow()
    )
}
