//! Handles stuff related to accounts, identity etc..

use std::str::FromStr;

use anyhow::Result;
use ed25519_dalek::{
    SecretKey, SigningKey,
    ed25519::signature::rand_core::{OsRng, RngCore},
};
use keyring::Entry;
use ucan::did::Ed25519Did;

type Did = String;
type Identity = Did;

/// Creates an `Identity` for given application
/// The keypair generated will be stored in OS secure storage
///
/// # Arguments
///
/// - `app`: The service for which Identity is made (for ex: tiles)
pub fn create_identity(app: &str) -> Result<Identity> {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let ed_did = Ed25519Did::from(signing_key.clone());
    let did = ed_did.to_string();
    let entry = Entry::new(app, &did)?;
    entry.set_secret(&signing_key.to_keypair_bytes())?;
    Ok(did)
}

/// Returns the `SecretKey` (ed25519_dalek type, but Private Key)
///
/// # Arguments
///
/// - `app`- The service for which Identity is made (for ex: tiles)
/// - `did` - The `Identity` of the service
pub fn get_secret_key(app: &str, did: &Identity) -> Result<SecretKey> {
    let entry = Entry::new(app, did)?;
    let mut bytes: [u8; 64] = [0u8; 64];
    let secret_pair = entry.get_secret()?;

    bytes[..64].copy_from_slice(secret_pair.as_slice());

    let signing_key = SigningKey::from_keypair_bytes(&bytes)?;
    Ok(signing_key.to_bytes())
}

pub fn get_public_key_from_did(did: &str) -> Result<[u8; 32]> {
    let ed_did = Ed25519Did::from_str(did)?;
    Ok(ed_did.0.to_bytes())
}

pub fn get_random_bytes() -> [u8; 16] {
    let mut value = [0u8; 16];
    OsRng.fill_bytes(&mut value);
    value
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
