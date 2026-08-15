/// Signer and guardian lifecycle management.
///
/// Handles adding/removing signers (passkeys, ed25519, session keys)
/// and guardians for social recovery. All mutations require owner auth
/// (enforced at the lib.rs level via `require_auth`).

use soroban_sdk::{Address, BytesN, Env, Symbol};

use novus_types::{
    GuardianInfo, SignerEntry, SignerType, WalletError,
};

use crate::storage::Storage;

/// Register a new signer in Persistent storage.
///
/// # Rules
/// - Cannot register a duplicate credential ID
/// - Guardian-type signers have weight 0 (cannot authorize normal txs)
pub fn add_signer(env: &Env, signer: SignerEntry) -> Result<(), WalletError> {
    // Check for duplicates
    if Storage::has_signer(env, &signer.credential_id) {
        return Err(WalletError::SignerAlreadyExists);
    }

    Storage::set_signer(env, &signer);

    env.events().publish(
        (Symbol::new(env, "signer_added"),),
        (signer.credential_id, signer.signer_type),
    );

    Ok(())
}

/// Remove a signer by credential ID.
///
/// # Rules
/// - Cannot remove the owner's primary passkey (prevents lockout)
/// - Guardian-type signers should be removed via `remove_guardian` instead
pub fn remove_signer(env: &Env, credential_id: &BytesN<32>) -> Result<(), WalletError> {
    // Ensure the signer exists
    let signer = Storage::get_signer(env, credential_id)?;

    // Prevent removing the owner's primary passkey
    let config = Storage::get_config(env)?;
    if *credential_id == config.owner_credential_id {
        return Err(WalletError::Unauthorized);
    }

    Storage::remove_signer(env, credential_id);

    env.events().publish(
        (Symbol::new(env, "signer_removed"),),
        (credential_id.clone(), signer.signer_type),
    );

    Ok(())
}

/// Register a new guardian for social recovery.
///
/// # Rules
/// - Cannot register duplicate guardians
/// - Updates the guardian_count in WalletConfig
pub fn add_guardian(env: &Env, guardian: GuardianInfo) -> Result<(), WalletError> {
    // Check for duplicates
    if Storage::has_guardian(env, &guardian.address) {
        return Err(WalletError::DuplicateGuardian);
    }

    let address = guardian.address.clone();
    let alias = guardian.alias.clone();

    Storage::set_guardian(env, &guardian);

    // Increment guardian count in config
    let mut config = Storage::get_config(env)?;
    config.guardian_count += 1;
    Storage::set_config(env, &config);

    env.events().publish(
        (Symbol::new(env, "guardian_added"),),
        (address, alias),
    );

    Ok(())
}

/// Remove a guardian from the registry.
///
/// # Rules
/// - Guardian must exist
/// - Updates the guardian_count in WalletConfig
/// - Caller should ensure remaining guardians still meet recovery threshold
pub fn remove_guardian(env: &Env, address: &Address) -> Result<(), WalletError> {
    // Verify guardian exists
    if !Storage::has_guardian(env, address) {
        return Err(WalletError::GuardianNotFound);
    }

    Storage::remove_guardian(env, address);

    // Decrement guardian count in config
    let mut config = Storage::get_config(env)?;
    config.guardian_count = config.guardian_count.saturating_sub(1);
    Storage::set_config(env, &config);

    env.events().publish(
        (Symbol::new(env, "guardian_removed"),),
        (address.clone(),),
    );

    Ok(())
}

/// Rotate the owner's primary credentials (final step of social recovery).
///
/// # Process
/// 1. Remove the old owner signer
/// 2. Register the new passkey as owner
/// 3. Update the owner_credential_id in WalletConfig
/// 4. Reset the nonce (prevents replay of old authorizations)
pub fn rotate_owner(
    env: &Env,
    new_credential_id: BytesN<32>,
    new_public_key: BytesN<65>,
) -> Result<(), WalletError> {
    let mut config = Storage::get_config(env)?;

    // Remove old owner signer
    Storage::remove_signer(env, &config.owner_credential_id);

    // Register new owner signer
    let new_signer = SignerEntry {
        signer_type: SignerType::Passkey,
        public_key: new_public_key,
        credential_id: new_credential_id.clone(),
        added_at: env.ledger().timestamp(),
        weight: 10,
    };
    Storage::set_signer(env, &new_signer);

    // Update config
    let old_credential = config.owner_credential_id.clone();
    config.owner_credential_id = new_credential_id.clone();
    Storage::set_config(env, &config);

    // Reset nonce to prevent replay of authorizations signed with old key
    Storage::set_nonce(env, 0);

    env.events().publish(
        (Symbol::new(env, "credentials_rotated"),),
        (old_credential, new_credential_id),
    );

    Ok(())
}
