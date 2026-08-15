/// Session key management.
///
/// Session keys are ephemeral ed25519 keypairs with scoped permissions.
/// They are stored in Temporary storage and auto-expire, eliminating
/// the need for explicit cleanup.

use soroban_sdk::{auth::Context, symbol_short, BytesN, Env, Symbol, Vec};

use novus_types::{
    SessionConfig, SignerEntry, SignerType, WalletError,
};

use crate::storage::Storage;
use crate::webauthn;

/// Create a new session key with scoped permissions.
///
/// # Returns
/// The session_id (SHA-256 of the session public key), used as the
/// credential_id for this session's SignerEntry.
pub fn create_session(
    env: &Env,
    session_public_key: BytesN<32>,
    config: SessionConfig,
) -> Result<BytesN<32>, WalletError> {
    // Derive session ID from the public key
    let session_id: BytesN<32> = env
        .crypto()
        .sha256(&session_public_key.clone().into())
        .into();

    // Calculate TTL from the config
    let current_ledger = env.ledger().sequence();
    if config.expires_at_ledger <= current_ledger {
        return Err(WalletError::SessionKeyInvalid);
    }
    let ttl_ledgers = config.expires_at_ledger - current_ledger;

    // Register as a signer (Persistent — needed for __check_auth lookup)
    let signer = SignerEntry {
        signer_type: SignerType::SessionKey,
        public_key: webauthn::pad_ed25519_key(env, &session_public_key),
        credential_id: session_id.clone(),
        added_at: env.ledger().timestamp(),
        weight: 1, // Lowest weight
    };
    Storage::set_signer(env, &signer);

    // Store session config in Temporary storage (auto-expires)
    Storage::set_session_config(env, &session_id, &config, ttl_ledgers);

    env.events().publish(
        (Symbol::new(env, "session_created"),),
        (session_id.clone(), config.expires_at_ledger),
    );

    Ok(session_id)
}

/// Revoke an active session key immediately.
///
/// Removes both the Persistent signer entry and the Temporary session config.
pub fn revoke_session(env: &Env, session_id: &BytesN<32>) -> Result<(), WalletError> {
    // Remove signer entry
    if !Storage::has_signer(env, session_id) {
        return Err(WalletError::SessionKeyInvalid);
    }
    Storage::remove_signer(env, session_id);
    Storage::remove_session(env, session_id);

    env.events().publish(
        (Symbol::new(env, "session_revoked"),),
        (session_id.clone(),),
    );

    Ok(())
}

/// Verify that a session key's usage is within its permitted scope.
///
/// Checks:
/// 1. Session config exists (not expired)
/// 2. Current ledger is before expiry
/// 3. Each invocation target is in the contract whitelist
/// 4. Each invocation function is in the function whitelist
/// 5. Cumulative spending is within the session total cap
///
/// # Returns
/// `Ok(())` if all checks pass, or a specific error describing the violation.
pub fn verify_session_scope(
    env: &Env,
    session_id: &BytesN<32>,
    contexts: &Vec<Context>,
) -> Result<(), WalletError> {
    // Load session config (returns Err if expired/deleted from Temporary)
    let mut session_config = Storage::get_session_config(env, session_id)?;

    // Explicit expiry check (belt-and-suspenders with TTL)
    if env.ledger().sequence() > session_config.expires_at_ledger {
        return Err(WalletError::SessionKeyExpired);
    }

    // Validate each invocation context against session scope
    for ctx in contexts.iter() {
        match ctx {
            Context::Contract(c) => {
                // Check contract whitelist (if non-empty)
                if !session_config.allowed_contracts.is_empty()
                    && !session_config.allowed_contracts.contains(&c.contract)
                {
                    return Err(WalletError::UnauthorizedTarget);
                }

                // Check function whitelist (if non-empty)
                if !session_config.allowed_functions.is_empty()
                    && !session_config.allowed_functions.contains(&c.fn_name)
                {
                    return Err(WalletError::UnauthorizedFunction);
                }

                // Extract and check transaction amount for token transfer calls
                if is_transfer_function(&c.fn_name) {
                    if let Some(amount) = extract_amount_from_args(env, &c.args) {
                        // Per-tx limit check
                        if amount > session_config.max_amount_per_tx {
                            return Err(WalletError::AmountExceedsSessionCap);
                        }

                        // Cumulative session limit check
                        let new_total = session_config.spent_amount + amount;
                        if new_total > session_config.max_total_amount {
                            return Err(WalletError::SessionTotalExceeded);
                        }

                        // Update running total
                        session_config.spent_amount = new_total;
                    }
                }
            }
            // CreateContractHostFn contexts are not allowed for session keys
            _ => return Err(WalletError::InvalidAuthContext),
        }
    }

    // Persist updated spent_amount
    let current_ledger = env.ledger().sequence();
    let remaining_ttl = session_config
        .expires_at_ledger
        .saturating_sub(current_ledger);
    Storage::set_session_config(env, session_id, &session_config, remaining_ttl);

    Ok(())
}

/// Check if a function name corresponds to a token transfer operation.
fn is_transfer_function(fn_name: &Symbol) -> bool {
    // We compare against known SAC (Stellar Asset Contract) function names
    // This avoids constructing new Symbols which costs CPU
    let transfer = symbol_short!("transfer");
    *fn_name == transfer
}

/// Extract the amount argument from a token transfer call.
///
/// SAC transfer signature: `transfer(from: Address, to: Address, amount: i128)`
/// The amount is at index 2.
fn extract_amount_from_args(env: &Env, args: &soroban_sdk::Vec<soroban_sdk::Val>) -> Option<i128> {
    use soroban_sdk::TryFromVal;
    if args.len() >= 3 {
        i128::try_from_val(env, &args.get(2)?).ok()
    } else {
        None
    }
}
