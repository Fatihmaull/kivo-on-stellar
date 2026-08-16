//! Nonce management for replay protection.
//!
//! Uses a dual-layer approach:
//! 1. **Sequential nonce** (Persistent) — monotonically increasing counter
//! 2. **Payload hash guard** (Temporary) — prevents exact payload replay within 24h

use soroban_sdk::{BytesN, Env};
use soroban_sdk::crypto::Hash;

use novus_types::{WalletError, WalletSignature};

use crate::storage::Storage;

/// Verify the nonce and set replay guards.
///
/// # Returns
/// - `Ok(())` if the nonce is valid and replay guards are set
/// - `Err(WalletError::InvalidNonce)` if the nonce doesn't match
/// - `Err(WalletError::ReplayDetected)` if the payload hash was already seen
pub fn verify_and_increment_nonce(
    env: &Env,
    signature: &WalletSignature,
    signature_payload: &Hash<32>,
) -> Result<(), WalletError> {
    // ── Layer 1: Sequential Nonce ──
    let stored_nonce = Storage::get_nonce(env);

    if signature.nonce != stored_nonce {
        return Err(WalletError::InvalidNonce);
    }

    // Atomically increment the nonce
    Storage::set_nonce(env, stored_nonce + 1);

    // ── Layer 2: Payload Hash Guard ──
    let payload_bytes = BytesN::from_array(env, &signature_payload.to_array());

    if Storage::has_replay_guard(env, &payload_bytes) {
        return Err(WalletError::ReplayDetected);
    }

    Storage::set_replay_guard(env, &payload_bytes);

    Ok(())
}
