/// Signer and guardian lifecycle management.
///
/// Handles adding/removing passkey and ed25519 signers, and guardians for
/// social recovery. All mutations require owner auth (enforced at the
/// lib.rs level via `require_auth`).
use soroban_sdk::{Address, BytesN, Env, Symbol};

use novus_types::{GuardianInfo, SignerEntry, SignerType, WalletError};

use crate::storage::Storage;

/// Register a new signer in Persistent storage.
///
/// # Rules
/// - Cannot register a duplicate credential ID
/// - Always stamped with the wallet's *current* credential epoch,
///   regardless of what the caller passed in `signer.epoch`
pub fn add_signer(env: &Env, mut signer: SignerEntry) -> Result<(), WalletError> {
    if Storage::has_signer(env, &signer.credential_id) {
        return Err(WalletError::SignerAlreadyExists);
    }

    let config = Storage::get_config(env)?;
    signer.epoch = config.credential_epoch;
    signer.added_at = env.ledger().timestamp();

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
pub fn remove_signer(env: &Env, credential_id: &BytesN<32>) -> Result<(), WalletError> {
    let signer = Storage::get_signer(env, credential_id)?;

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
    if Storage::has_guardian(env, &guardian.address) {
        return Err(WalletError::DuplicateGuardian);
    }

    let address = guardian.address.clone();
    let alias = guardian.alias.clone();

    Storage::set_guardian(env, &guardian);

    let mut config = Storage::get_config(env)?;
    config.guardian_count += 1;
    Storage::set_config(env, &config);

    env.events()
        .publish((Symbol::new(env, "guardian_added"),), (address, alias));

    Ok(())
}

/// Remove a guardian from the registry.
///
/// # Rules
/// - Guardian must exist
/// - Refuses to drop the guardian count below `recovery_threshold` — doing
///   so would make social recovery permanently impossible to trigger.
pub fn remove_guardian(env: &Env, address: &Address) -> Result<(), WalletError> {
    if !Storage::has_guardian(env, address) {
        return Err(WalletError::GuardianNotFound);
    }

    let mut config = Storage::get_config(env)?;
    if config.guardian_count.saturating_sub(1) < config.recovery_threshold {
        return Err(WalletError::BelowRecoveryThreshold);
    }

    Storage::remove_guardian(env, address);

    config.guardian_count -= 1;
    Storage::set_config(env, &config);

    env.events()
        .publish((Symbol::new(env, "guardian_removed"),), (address.clone(),));

    Ok(())
}

/// Rotate the owner's primary credentials (final step of social recovery).
///
/// # Process
/// 1. Register the new passkey as owner, under a **bumped** credential
///    epoch
/// 2. Update `owner_credential_id` and `credential_epoch` in WalletConfig
/// 3. Reset the nonce (prevents replay of old authorizations)
///
/// The epoch bump is what makes this a real recovery rather than a partial
/// one: every signer and session key registered before this call — the
/// legitimate owner's backups *and* anything an attacker who triggered the
/// need for recovery may have quietly added — stops authenticating,
/// because its stored `epoch` no longer matches
/// `config.credential_epoch`. Nothing needs to be enumerated or deleted;
/// old entries simply go inert and age out of Persistent storage on their
/// own once nothing ever refreshes their TTL again. The old owner signer
/// entry is left in place (also inert) rather than removed, since removing
/// it costs a write this call doesn't need.
pub fn rotate_owner(
    env: &Env,
    new_credential_id: BytesN<32>,
    new_public_key: BytesN<65>,
) -> Result<(), WalletError> {
    let mut config = Storage::get_config(env)?;

    let old_credential = config.owner_credential_id.clone();
    let new_epoch = config.credential_epoch + 1;

    let new_signer = SignerEntry {
        signer_type: SignerType::Passkey,
        public_key: new_public_key,
        credential_id: new_credential_id.clone(),
        added_at: env.ledger().timestamp(),
        weight: 10,
        epoch: new_epoch,
    };
    Storage::set_signer(env, &new_signer);

    config.owner_credential_id = new_credential_id.clone();
    config.credential_epoch = new_epoch;
    Storage::set_config(env, &config);

    // Reset nonce to prevent replay of authorizations signed with old key
    Storage::set_nonce(env, 0);

    env.events().publish(
        (Symbol::new(env, "credentials_rotated"),),
        (old_credential, new_credential_id, new_epoch),
    );

    Ok(())
}
