// Configuration related stuff

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::str::FromStr;
use std::{env, fs};

pub trait ConfigProvider {
    fn get_config_dir(&self) -> Result<PathBuf>;
    fn get_data_dir(&self) -> Result<PathBuf>;
    fn get_lib_dir(&self) -> Result<PathBuf>;
}

#[derive(Debug, Default)]
pub struct DefaultProvider;

impl ConfigProvider for DefaultProvider {
    fn get_config_dir(&self) -> Result<PathBuf> {
        if cfg!(debug_assertions) {
            let base_dir = env::current_dir().context("Failed to fetch CURRENT_DIR")?;
            Ok(base_dir.join(".tiles_dev/tiles"))
        } else {
            let home_dir = env::home_dir().context("Failed to fetch $HOME")?;
            let config_dir = match env::var("XDG_CONFIG_HOME") {
                Ok(val) => PathBuf::from(val),
                Err(_err) => home_dir.join(".config"),
            };
            Ok(config_dir.join("tiles"))
        }
    }

    fn get_data_dir(&self) -> Result<PathBuf> {
        if cfg!(debug_assertions) {
            let base_dir = env::current_dir().context("Failed to fetch CURRENT_DIR")?;
            Ok(base_dir.join(".tiles_dev/tiles"))
        } else {
            let home_dir = env::home_dir().context("Failed to fetch $HOME")?;
            let data_dir = match env::var("XDG_DATA_HOME") {
                Ok(val) => PathBuf::from(val),
                Err(_err) => home_dir.join(".local/share"),
            };
            Ok(data_dir.join("tiles"))
        }
    }

    fn get_lib_dir(&self) -> Result<PathBuf> {
        if cfg!(debug_assertions) {
            let base_dir = env::current_dir().context("Failed to fetch CURRENT_DIR")?;
            Ok(base_dir)
        } else {
            let home_dir = env::home_dir().context("Failed to fetch $HOME")?;
            let data_dir = home_dir.join(".local/lib");
            Ok(data_dir.join("tiles"))
        }
    }
}
pub fn set_memory_path(path: &str) -> Result<String> {
    set_memory_path_with_provider(&DefaultProvider, path)
}

pub fn get_memory_path() -> Result<String> {
    let tiles_config_dir = DefaultProvider.get_config_dir()?;
    let mut is_memory_path_found: bool = false;
    let mut memory_path: String = String::from("");
    if tiles_config_dir.is_dir()
        && let Ok(content) = fs::read_to_string(tiles_config_dir.join(".memory_path"))
    {
        memory_path = content;
        is_memory_path_found = true;
    }

    if is_memory_path_found {
        Ok(memory_path)
    } else {
        Err(anyhow::anyhow!(format!("NOT SET")))
    }
}

pub fn get_default_memory_path() -> Result<PathBuf> {
    let tiles_data_dir = DefaultProvider.get_data_dir()?;
    let memory_path = tiles_data_dir.join("memory");
    Ok(memory_path)
}

pub fn create_default_memory_folder() -> Result<PathBuf> {
    let memory_path = get_default_memory_path()?;
    fs::create_dir_all(&memory_path).context("Failed to create tiles memory directory")?;
    Ok(memory_path)
}

pub fn is_memory_model(modelname: &str) -> bool {
    if modelname.contains("mem") {
        return true;
    }
    false
}

fn set_memory_path_with_provider<P: ConfigProvider>(provider: &P, path: &str) -> Result<String> {
    let path_buf = PathBuf::from_str(path)?;
    if path_buf.try_exists()? {
        let tiles_config_dir = provider.get_config_dir()?;
        let tiles_config_memory_path = tiles_config_dir.join(".memory_path");
        if tiles_config_memory_path.try_exists()? {
            fs::write(tiles_config_memory_path, path)?;
        } else {
            fs::create_dir_all(&tiles_config_dir)
                .context("Failed to create tiles config directory")?;
            fs::write(tiles_config_memory_path, path)
                .context("Failed to write the memory path to .memory_path")?;
        }
    } else {
        return Err(anyhow::anyhow!(format!(
            "Not a valid path {}",
            path_buf.to_str().unwrap()
        )));
    }
    Ok(format!(
        "Memory path set successfully at {}",
        path_buf.to_str().unwrap()
    ))
}
