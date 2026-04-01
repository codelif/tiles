// Contains functions for health checking various dependencies

use std::env;

use anyhow::Result;

use crate::{
    daemon::ping,
    utils::installer::{UpdateInfo, get_update_info},
};

pub async fn check_health() -> Result<()> {
    let os = env::consts::OS;
    println!("Running diagnosis...\n");

    println!("Checking for newer version...\n");
    let update_info: UpdateInfo = get_update_info().await?;
    if update_info.can_update {
        println!("⚠️ Outdated version, try `tiles update`");
        let update_str = format!(
            "\tcurrent version - {}\n\tlatest version - {}",
            update_info.current_version, update_info.latest_version
        );
        println!("{}", update_str);
    } else {
        println!("✅ Running latest version\n");
    }
    if os == "macos" && !cfg!(debug_assertions) {
        check_server_status().await;
    }
    Ok(())
}

async fn check_server_status() {
    if ping(None).await.is_ok() {
        println!("✅ Daemon is UP")
    } else {
        println!("❌ Daemon is DOWN, try `tiles daemon start`")
    }
}
