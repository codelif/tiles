/// Manages model snapshot downloading from HuggingFace
use std::{fs, path::PathBuf};

use anyhow::{Result, anyhow};
use hf_hub::api::{
    Siblings,
    tokio::{ApiBuilder, ApiError},
};

use crate::utils::config::{ConfigProvider, DefaultProvider};

/// Download the entire model (including snapshot) for the given model name
pub async fn pull_model(model_name: &str) -> Result<()> {
    snapshot_download(model_name).await
}

pub async fn snapshot_download(modelname: &str) -> Result<()> {
    let allow_patterns = [
        ".json",
        ".txt",
        ".safetensors",
        ".md",
        ".gitattributes",
        "LICENSE",
    ];
    let api_build_result = ApiBuilder::new()
        .with_progress(true)
        .with_cache_dir(get_model_download_path()?)
        .build();

    match api_build_result {
        Ok(api) => {
            let repo = api.model(modelname.to_owned());
            match repo.info().await {
                Ok(repo_info) => {
                    let filtered_siblings = repo_info
                        .siblings
                        .iter()
                        .filter(|sibling| {
                            allow_patterns
                                .iter()
                                .any(|pat| sibling.rfilename.ends_with(pat))
                        })
                        .collect::<Vec<&Siblings>>();

                    for sibling in filtered_siblings {
                        if repo.get(&sibling.rfilename).await.is_err() {
                            return Err(anyhow!(
                                "{:?} failed to download, retry again",
                                &sibling.rfilename,
                            ));
                        }
                    }
                }
                Err(err) => return Err(anyhow!(format_hf_api_error(err))),
            };
        }
        Err(err) => return Err(anyhow!(format_hf_api_error(err))),
    }

    Ok(())
}

fn format_hf_api_error(api_error: ApiError) -> String {
    match api_error {
        ApiError::RequestError(err) => err.to_string(),
        ApiError::TooManyRetries(err) => err.to_string(),
        _err => "Something unexpected happened, check your internet connection".to_owned(),
    }
}

fn get_model_download_path() -> Result<PathBuf> {
    let data_dir = DefaultProvider.get_user_data_dir()?;
    let model_dir = data_dir.join("models/huggingface/hub");
    if !model_dir.exists() {
        fs::create_dir_all(&model_dir)?;
    }
    Ok(model_dir)
}

#[cfg(test)]
mod tests {}
