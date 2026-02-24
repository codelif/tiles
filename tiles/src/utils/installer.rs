// Stuff related to auto installing tiles

// TODO: checklist to finish the feat
// fn for getting latest version - DONE
// fn for checking if update needed - DONE
// run the curl script - DONE
// run on bare tiles command
// refactor, tests, and lets goooo
use std::{
    process::{Command, Stdio},
    str::FromStr,
    time::Duration,
};

use anyhow::{Result, anyhow};
use reqwest::{Client, Url, header::HeaderMap};
use semver::{Version, VersionReq};
use serde::Deserialize;
use serde_json::json;

const RELEASES_BASE_ENDPOINT: &str = "https://api.github.com";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

pub async fn try_update() -> Result<String> {
    let latest_vsn = get_latest_version(RELEASES_BASE_ENDPOINT).await?;

    let req_vsn = VersionReq::parse(&latest_vsn)?;
    let current_vsn = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|e| anyhow!("Failed to parse pkg version due to {}", e))?;

    //TODO: maybe can add a release check, so can run a test for this?
    if req_vsn.matches(&current_vsn) {
        Ok("Already latest version".to_owned())
    } else {
        let mut child = Command::new("curl")
            .arg("-fsSL")
            .arg("https://tiles.run/install.sh")
            .stdout(Stdio::piped())
            .spawn()?;

        let run_sh_cmd = Command::new("sh")
            .stdin(child.stdout.take().unwrap())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        Ok("Update completed".to_owned())
    }
}

pub async fn get_latest_version(base_url: &str) -> Result<String> {
    let mut headers = HeaderMap::new();
    headers.insert("X-GitHub-Api-Version", "2022-11-28".parse().unwrap());
    headers.insert("Accept", "application/vnd.github+json".parse().unwrap());
    headers.insert("user-agent", "Tiles".parse().unwrap());
    let client_builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .default_headers(headers);

    let client = client_builder.build()?;
    let res = client
        .get(format!(
            "{}/repos/tilesprivacy/tiles/releases/latest",
            base_url
        ))
        .send()
        .await;

    match res {
        Err(err) => {
            println!("err {:?}", err);
            Err(anyhow!("request failed"))
        }
        _ => {
            let release_data: Release = res?.json::<Release>().await?;
            println!("{}", release_data.tag_name);
            Ok(release_data.tag_name)
        }
    }
}

#[cfg(test)]

mod tests {
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;

    #[tokio::test]
    async fn test_get_latest_version() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/tilesprivacy/tiles/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(
                {
                    "tag_name": "0.4.1"
                }
            )))
            .mount(&mock_server)
            .await;

        let tag = get_latest_version(mock_server.uri().as_str())
            .await
            .unwrap();
        assert_eq!(tag, "0.4.1".to_owned())
    }
}
