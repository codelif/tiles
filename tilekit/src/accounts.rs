// Handles stuff related to accounts, identity etc..
use anyhow::Result;
use ed25519_dalek::{SigningKey, ed25519::signature::rand_core::OsRng};
use keyring::Entry;
use ucan::did::Ed25519Did;

type DID = String;
type Identity = DID;

/// Creates an `Identity`
/// # Parameters
/// - `app`: The service for which Identity is made (for ex: tiles)
/// The keypair generated will be stored in OS secure storage
/// Returns an `Identity`
pub fn create_identity(app: &str) -> Result<Identity> {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let ed_did = Ed25519Did::from(signing_key.clone());
    let did = ed_did.to_string();
    let entry = Entry::new(app, &did)?;
    entry.set_secret(&signing_key.to_keypair_bytes())?;
    Ok(did)
}

#[cfg(test)]
mod tests {
    use keyring::{mock, set_default_credential_builder};

    use super::*;

    #[test]
    fn test_create_success() -> Result<()> {
        set_default_credential_builder(mock::default_credential_builder());
        let did = create_identity("tiles")?;
        assert!(did.starts_with("did:key"));
        Ok(())
    }
}
