/// Session key management.
///
/// Session keys are ephemeral ed25519 keypairs with scoped permissions.
/// Everything about one — its public key, its scope, its running spend —
/// lives in a single Temporary storage record that expires on its own.
/// There is deliberately no Persistent entry anywhere in this flow: nothing
/// about a session should be able to outlive its own TTL.
use soroban_sdk::{auth::Context, symbol_short, BytesN, Env, Symbol, Vec};

use novus_types::{SessionConfig, WalletError};

use crate::storage::Storage;
use crate::webauthn;

/// Create a new session key with scoped permissions.
///
/// # Rules (fail-closed by construction)
/// - `allowed_contracts` and `allowed_functions` must both be non-empty.
///   A session key with no declared scope is refused at creation rather
///   than silently becoming a "do anything" key at use time.
/// - The wallet's own address may never be whitelisted — a session key
///   must never be able to call back into `add_signer`, `rotate_owner`, or
///   any other owner-only method on this contract.
///
/// # Returns
/// The session_id (SHA-256 of the session public key), used as the
/// `credential_id` for this session in `WalletSignature`.
pub fn create_session(
    env: &Env,
    session_public_key: BytesN<32>,
    mut config: SessionConfig,
) -> Result<BytesN<32>, WalletError> {
    if config.allowed_contracts.is_empty() || config.allowed_functions.is_empty() {
        return Err(WalletError::SessionKeyInvalid);
    }
    if config.allowed_contracts.contains(env.current_contract_address()) {
        return Err(WalletError::UnauthorizedTarget);
    }
    if config.max_total_amount < config.max_amount_per_tx {
        return Err(WalletError::SessionKeyInvalid);
    }

    let session_id: BytesN<32> = env.crypto().sha256(&session_public_key.clone().into()).into();
    if Storage::has_session(env, &session_id) {
        return Err(WalletError::SessionKeyInvalid);
    }

    let current_ledger = env.ledger().sequence();
    if config.expires_at_ledger <= current_ledger {
        return Err(WalletError::SessionKeyInvalid);
    }
    let ttl_ledgers = config.expires_at_ledger - current_ledger;

    let wallet_config = Storage::get_config(env)?;
    config.public_key = session_public_key;
    config.spent_amount = 0;
    config.epoch = wallet_config.credential_epoch;

    Storage::set_session_config(env, &session_id, &config, ttl_ledgers);

    env.events().publish(
        (Symbol::new(env, "session_created"),),
        (session_id.clone(), config.expires_at_ledger),
    );

    Ok(session_id)
}

/// Revoke an active session key immediately.
pub fn revoke_session(env: &Env, session_id: &BytesN<32>) -> Result<(), WalletError> {
    if !Storage::has_session(env, session_id) {
        return Err(WalletError::SessionKeyInvalid);
    }
    Storage::remove_session(env, session_id);

    env.events()
        .publish((Symbol::new(env, "session_revoked"),), (session_id.clone(),));

    Ok(())
}

/// Verify an ed25519 signature and scope for a session key, then apply its
/// spending state.
///
/// # Checks
/// 1. Session exists (not expired / not revoked)
/// 2. Session was created under the wallet's *current* credential epoch —
///    a recovery invalidates in-flight session keys the same as any other
///    credential
/// 3. Signature verifies against the session's public key
/// 4. Every invocation context's target contract AND function are in the
///    session's whitelist — absence from either list denies (fail-closed)
/// 5. Cumulative + per-tx spending stays within the session's caps
pub fn verify_session(
    env: &Env,
    session_id: &BytesN<32>,
    payload: &soroban_sdk::crypto::Hash<32>,
    signature_bytes: &BytesN<64>,
    contexts: &Vec<Context>,
) -> Result<(), WalletError> {
    let mut session_config = Storage::get_session_config(env, session_id)?;

    if env.ledger().sequence() > session_config.expires_at_ledger {
        return Err(WalletError::SessionKeyExpired);
    }

    let wallet_config = Storage::get_config(env)?;
    if session_config.epoch != wallet_config.credential_epoch {
        return Err(WalletError::StaleCredentialEpoch);
    }

    webauthn::verify_ed25519_session_signature(env, &session_config.public_key, payload, signature_bytes);

    for ctx in contexts.iter() {
        match ctx {
            Context::Contract(c) => {
                // The wallet's own address may never appear in a session's
                // context list, whitelist or not — this is what stops a
                // session key from ever reaching an owner-only method.
                if c.contract == env.current_contract_address() {
                    return Err(WalletError::UnauthorizedTarget);
                }
                if !session_config.allowed_contracts.contains(&c.contract) {
                    return Err(WalletError::UnauthorizedTarget);
                }
                if !session_config.allowed_functions.contains(&c.fn_name) {
                    return Err(WalletError::UnauthorizedFunction);
                }

                if is_transfer_function(&c.fn_name) {
                    if let Some(amount) = extract_amount_from_args(env, &c.args) {
                        if amount > session_config.max_amount_per_tx {
                            return Err(WalletError::AmountExceedsSessionCap);
                        }
                        let new_total = session_config.spent_amount + amount;
                        if new_total > session_config.max_total_amount {
                            return Err(WalletError::SessionTotalExceeded);
                        }
                        session_config.spent_amount = new_total;
                    }
                }
            }
            // CreateContractHostFn contexts are not allowed for session keys
            _ => return Err(WalletError::InvalidAuthContext),
        }
    }

    let current_ledger = env.ledger().sequence();
    let remaining_ttl = session_config
        .expires_at_ledger
        .saturating_sub(current_ledger);
    Storage::set_session_config(env, session_id, &session_config, remaining_ttl);

    Ok(())
}

fn is_transfer_function(fn_name: &Symbol) -> bool {
    let transfer = symbol_short!("transfer");
    *fn_name == transfer
}

/// SAC transfer signature: `transfer(from: Address, to: Address, amount: i128)`
fn extract_amount_from_args(env: &Env, args: &soroban_sdk::Vec<soroban_sdk::Val>) -> Option<i128> {
    use soroban_sdk::TryFromVal;
    if args.len() >= 3 {
        i128::try_from_val(env, &args.get(2)?).ok()
    } else {
        None
    }
}
