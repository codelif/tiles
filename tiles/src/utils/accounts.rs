// Stuff related to account and identity system

use std::collections::HashMap;

use anyhow::Result;
use tilekit::accounts::create_identity;
use toml::Table;

use crate::utils::config::save_config;
const ROOT_USER_CONFIG_KEY: &str = "root-user";

//TODO: add docs
pub fn get_root_user_details(config: &Table) -> Result<HashMap<String, String>> {
    Ok(get_root_account(&config))
}

fn get_root_account(config: &Table) -> HashMap<String, String> {
    let root_user = config.get(ROOT_USER_CONFIG_KEY).unwrap();
    let root_user_table = root_user.as_table().unwrap();
    let mut root_user_map = HashMap::new();
    for ele in root_user_table {
        root_user_map.insert(ele.0.to_string(), ele.1.as_str().unwrap().to_owned());
    }
    root_user_map
}

// Lets return main config table only, let the caller do whatever it wants...
// TODO: Needed docs
pub fn create_root_account(config: &Table, nickname: Option<String>) -> Result<Table> {
    let root_user = config.get(ROOT_USER_CONFIG_KEY).unwrap();
    let root_user_table = root_user.as_table().unwrap();
    let did = root_user_table.get("id").unwrap().as_str().unwrap();
    if did.is_empty() {
        let root_user_config = create_root_user(root_user_table, nickname)?;
        Ok(root_user_config)
    } else {
        Ok(root_user_table.clone())
    }
}

//TODO: docs
pub fn save_root_account(mut config: Table, root_user_config: &Table) -> Result<()> {
    config.insert(
        String::from(ROOT_USER_CONFIG_KEY),
        toml::Value::Table(root_user_config.clone()),
    );
    save_config(&config)
}

// TODO: add docs
pub fn set_nickname(config: &Table, nickname: String) -> Result<Table> {
    let root_user = config.get(ROOT_USER_CONFIG_KEY).unwrap();
    let mut root_user_table = root_user.as_table().unwrap().clone();
    let did = root_user_table.get("id").unwrap().as_str().unwrap();
    if did.is_empty() {
        Err(anyhow::anyhow!("No Root user available"))
    } else {
        root_user_table.insert("id".to_owned(), toml::Value::String(did.to_owned()));
        root_user_table.insert("nickname".to_owned(), toml::Value::String(nickname));
        Ok(root_user_table)
    }
}

// TODO: add docs
fn create_root_user(root_user_config: &Table, nickname: Option<String>) -> Result<Table> {
    // get root user details
    let mut root_user_table = root_user_config.clone();
    match create_identity("tiles") {
        Ok(did) => {
            root_user_table.insert("id".to_owned(), toml::Value::String(did));
            if nickname.is_some() {
                root_user_table.insert(
                    "nickname".to_owned(),
                    toml::Value::String(nickname.unwrap()),
                );
            }
            Ok(root_user_table)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]

mod tests {
    use keyring::{mock, set_default_credential_builder};
    use toml::Table;

    use crate::utils::accounts::{create_root_account, get_root_account};

    #[test]
    fn test_get_root_user_details_empty_id() {
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = ''
                nickname = ''
            "#,
        )
        .unwrap();
        let acc_details = get_root_account(&config);
        assert!(acc_details.get("id").unwrap().is_empty());
    }

    #[test]
    fn test_get_root_user_details_valid_id() {
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = 'did:key:xyz'
                nickname = ''
            "#,
        )
        .unwrap();
        let acc_details = get_root_account(&config);
        assert!(acc_details.get("id").unwrap().contains("did:key"));
    }

    #[test]
    fn test_create_root_account_but_exists() {
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = 'did:key:xyz'
                nickname = ''
            "#,
        )
        .unwrap();
        let root_user = create_root_account(&config, None).unwrap();

        assert_eq!(
            root_user.get("id").unwrap().as_str().unwrap(),
            "did:key:xyz"
        );
    }

    #[test]
    fn test_create_root_account_new() {
        set_default_credential_builder(mock::default_credential_builder());
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = ''
                nickname = ''
            "#,
        )
        .unwrap();
        let root_user = create_root_account(&config, None).unwrap();

        assert_ne!(
            root_user.get("id").unwrap().as_str().unwrap(),
            "did:key:xyz"
        );

        assert!(
            root_user
                .get("id")
                .unwrap()
                .as_str()
                .unwrap()
                .starts_with("did:key")
        );
    }

    #[test]
    fn test_create_root_account_new_w_nickname() {
        set_default_credential_builder(mock::default_credential_builder());
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = ''
                nickname = ''
            "#,
        )
        .unwrap();
        let root_user = create_root_account(&config, Some(String::from("madclaws"))).unwrap();

        assert_ne!(
            root_user.get("id").unwrap().as_str().unwrap(),
            "did:key:xyz"
        );

        assert!(
            root_user
                .get("id")
                .unwrap()
                .as_str()
                .unwrap()
                .starts_with("did:key")
        );

        assert_eq!(
            root_user.get("nickname").unwrap().as_str().unwrap(),
            "madclaws"
        );
    }
}
