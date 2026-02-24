// Stuff related to auto installing tiles

// TODO: checklist to finish the feat
// fn for getting latest version - DONE
// fn for checking if update needed
// run the curl script
// refactor, tests, and lets goooo
use std::{str::FromStr, time::Duration};

use anyhow::{Result, anyhow};
use reqwest::{Client, Url, header::HeaderMap};
use serde::Deserialize;
use serde_json::json;

const RELEASES_BASE_ENDPOINT: &str = "https://api.github.com";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
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
