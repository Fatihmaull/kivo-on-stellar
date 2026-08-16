#![no_std]

mod auth;
mod nonce;
mod policy;
mod session;
mod signers;
mod storage;
mod webauthn;

use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contractimpl,
    crypto::Hash,
    Address, BytesN, Env, IntoVal, Symbol, Val, Vec,
};

use novus_types::{
    GuardianInfo, SessionConfig, SignerEntry, WalletConfig, WalletError, WalletSignature,
};

use crate::storage::Storage;

/// ═══════════════════════════════════════════════════════════════════════
/// Kivo SmartAccount Contract
/// ═══════════════════════════════════════════════════════════════════════
///
/// The core account abstraction contract. When deployed, this contract's
/// address becomes the user's "wallet address." All token balances,
/// authorizations, and identity are tied to this contract.
///
/// Key responsibilities:
/// - Authentication via `__check_auth` (WebAuthn/Passkeys, ed25519, session keys)
/// - Signer lifecycle management (add/remove/rotate credentials)
/// - Nonce-based replay protection
/// - Policy enforcement (spending limits, whitelists, via PolicyEngine)
/// - Session key management for dApp interactions
/// - Guardian management for social recovery
/// - Sponsored (gasless) execution via a registered Paymaster
#[contract]
pub struct SmartAccountContract;

#[contractimpl]
impl SmartAccountContract {
    // ═══════════════════════════════════════════════════════════════════
    // CONSTRUCTOR
    // ═══════════════════════════════════════════════════════════════════

    /// Deploy-time constructor. Soroban invokes this atomically with
    /// contract creation (`stellar contract deploy ... -- --owner_credential_id ...`),
    /// so there is no window between "contract exists" and "contract has an
    /// owner" for anyone else to claim it — the classic separate-`initialize()`
    /// front-running gap simply doesn't exist here.
    ///
    /// # Arguments
    /// * `owner_credential_id` - WebAuthn credential ID from passkey registration
    /// * `owner_public_key` - 65-byte uncompressed secp256r1 public key (0x04 || x || y)
    /// * `rp_id_hash` - SHA-256 of the WebAuthn Relying Party ID (origin) this
    ///   wallet's passkeys are bound to
    /// * `recovery_threshold` - Number of guardian approvals needed for recovery (m)
    /// * `recovery_timelock_ledgers` - Delay in ledgers before recovery executes
    /// * `default_daily_limit` - Default per-token daily spending limit (in base units)
    pub fn __constructor(
        env: Env,
        owner_credential_id: BytesN<32>,
        owner_public_key: BytesN<65>,
        rp_id_hash: BytesN<32>,
        recovery_threshold: u32,
        recovery_timelock_ledgers: u32,
        default_daily_limit: i128,
    ) {
        let config = WalletConfig {
            version: 1,
            owner_credential_id: owner_credential_id.clone(),
            rp_id_hash,
            recovery_threshold,
            recovery_timelock_ledgers,
            guardian_count: 0,
            default_daily_limit,
            credential_epoch: 0,
        };
        Storage::set_config(&env, &config);

        let owner_signer = SignerEntry {
            signer_type: novus_types::SignerType::Passkey,
            public_key: owner_public_key,
            credential_id: owner_credential_id,
            added_at: env.ledger().timestamp(),
            weight: 10,
            epoch: 0,
        };
        Storage::set_signer(&env, &owner_signer);

        Storage::set_nonce(&env, 0);
        Storage::set_initialized(&env, true);

        env.storage().instance().extend_ttl(103_680, 103_680); // ~6 days
    }

    // ═══════════════════════════════════════════════════════════════════
    // PUBLIC READ METHODS
    // ═══════════════════════════════════════════════════════════════════

    pub fn get_config(env: Env) -> Result<WalletConfig, WalletError> {
        Storage::get_config(&env)
    }

    /// Get the current nonce value (useful for SDK to build transactions).
    pub fn get_nonce(env: Env) -> Result<u128, WalletError> {
        if !Storage::is_initialized(&env) {
            return Err(WalletError::NotInitialized);
        }
        Ok(Storage::get_nonce(&env))
    }

    pub fn is_signer(env: Env, credential_id: BytesN<32>) -> bool {
        Storage::has_signer(&env, &credential_id)
    }

    pub fn get_signer(env: Env, credential_id: BytesN<32>) -> Result<SignerEntry, WalletError> {
        Storage::get_signer(&env, &credential_id)
    }

    pub fn is_guardian(env: Env, address: Address) -> bool {
        Storage::has_guardian(&env, &address)
    }

    pub fn get_guardian(env: Env, address: Address) -> Result<GuardianInfo, WalletError> {
        Storage::get_guardian(&env, &address)
    }

    // ═══════════════════════════════════════════════════════════════════
    // SIGNER MANAGEMENT (requires owner auth)
    // ═══════════════════════════════════════════════════════════════

    /// Register a new backup signer (passkey or ed25519).
    /// Requires authentication from the contract itself (triggers __check_auth).
    pub fn add_signer(env: Env, signer_entry: SignerEntry) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();
        signers::add_signer(&env, signer_entry)
    }

    /// Remove a signer by credential ID. Cannot remove the owner passkey.
    pub fn remove_signer(env: Env, credential_id: BytesN<32>) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();
        signers::remove_signer(&env, &credential_id)
    }

    // ═══════════════════════════════════════════════════════════════════
    // GUARDIAN MANAGEMENT (requires owner auth)
    // ═══════════════════════════════════════════════════════════════

    pub fn add_guardian(env: Env, guardian: GuardianInfo) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();
        signers::add_guardian(&env, guardian)
    }

    pub fn remove_guardian(env: Env, guardian_address: Address) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();
        signers::remove_guardian(&env, &guardian_address)
    }

    // ═══════════════════════════════════════════════════════════════════
    // SESSION KEY MANAGEMENT
    // ═══════════════════════════════════════════════════════════════

    /// Create a new ephemeral session key with scoped permissions.
    /// Returns the session ID (derived from the session public key).
    ///
    /// `config.public_key`, `config.spent_amount`, and `config.epoch` are
    /// overwritten by the contract regardless of what is passed in — the
    /// caller only meaningfully controls the scope and cap fields.
    pub fn create_session(
        env: Env,
        session_public_key: BytesN<32>,
        config: SessionConfig,
    ) -> Result<BytesN<32>, WalletError> {
        env.current_contract_address().require_auth();
        session::create_session(&env, session_public_key, config)
    }

    pub fn revoke_session(env: Env, session_id: BytesN<32>) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();
        session::revoke_session(&env, &session_id)
    }

    /// Read a live session's current scope and running spend. Returns
    /// `SessionKeyExpired` once the session's Temporary entry has expired
    /// or been revoked.
    pub fn get_session(env: Env, session_id: BytesN<32>) -> Result<SessionConfig, WalletError> {
        Storage::get_session_config(&env, &session_id)
    }

    // ═══════════════════════════════════════════════════════════════════
    // MODULE WIRING (requires owner auth)
    // ═══════════════════════════════════════════════════════════════

    /// Register the social RecoveryModule contract address.
    pub fn set_recovery_module(env: Env, recovery_module: Address) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();
        Storage::set_recovery_module(&env, &recovery_module);
        Ok(())
    }

    /// Register a PolicyEngine contract for whitelist + per-token limit
    /// enforcement on top of the built-in daily limit. Optional — if never
    /// set, only the built-in limit applies.
    pub fn set_policy_engine(env: Env, policy_engine: Address) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();
        Storage::set_policy_engine(&env, &policy_engine);
        Ok(())
    }

    /// Register a Paymaster contract for `execute_sponsored`.
    pub fn set_paymaster(env: Env, paymaster: Address) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();
        Storage::set_paymaster(&env, &paymaster);
        Ok(())
    }

    pub fn get_policy_engine(env: Env) -> Option<Address> {
        Storage::get_policy_engine(&env)
    }

    pub fn get_paymaster(env: Env) -> Option<Address> {
        Storage::get_paymaster(&env)
    }

    // ═══════════════════════════════════════════════════════════════════
    // SPONSORED EXECUTION (gas abstraction)
    // ═══════════════════════════════════════════════════════════════

    /// Execute `target.fn_name(args)` and pay the relayer's quoted fee in
    /// one token, atomically, under a single wallet signature.
    ///
    /// Both the target invocation and the fee transfer are sub-invocations
    /// of *this* call, so they share one `SorobanAuthorizationEntry`: the
    /// user signs once over a tree that already contains both actions, and
    /// either both succeed or the entire transaction reverts — there is no
    /// state where the sponsored action lands but the relayer goes unpaid,
    /// or vice versa.
    ///
    /// `max_fee` is part of what the wallet's own signature covers (it's a
    /// plain argument to this authorized call), so the relayer cannot
    /// charge more than the user actually agreed to. `quoted` is what
    /// actually gets pulled — it must clear the same `<= max_fee` bound.
    ///
    /// # Errors
    /// - `FeeExceedsMax` if `quoted > max_fee`
    /// - `UnsupportedFeeToken` if `fee_token` isn't accepted by the
    ///   registered Paymaster
    /// - `NotInitialized` if no Paymaster has been registered
    pub fn execute_sponsored(
        env: Env,
        target: Address,
        fn_name: Symbol,
        args: Vec<Val>,
        fee_token: Address,
        max_fee: i128,
        quoted: i128,
    ) -> Result<Val, WalletError> {
        env.current_contract_address().require_auth();

        if quoted > max_fee {
            return Err(WalletError::FeeExceedsMax);
        }

        let paymaster = Storage::get_paymaster(&env).ok_or(WalletError::NotInitialized)?;
        let accepted: bool = env.invoke_contract(
            &paymaster,
            &Symbol::new(&env, "is_accepted_token"),
            (fee_token.clone(),).into_val(&env),
        );
        if !accepted {
            return Err(WalletError::UnsupportedFeeToken);
        }

        // 1 — the action the user actually wanted.
        let result: Val = env.invoke_contract(&target, &fn_name, args);

        // 2 — the fee, same call tree, same signature.
        env.invoke_contract::<()>(
            &fee_token,
            &Symbol::new(&env, "transfer"),
            (
                env.current_contract_address(),
                paymaster.clone(),
                quoted,
            )
                .into_val(&env),
        );

        env.events().publish(
            (Symbol::new(&env, "sponsored_execution"),),
            (target, fee_token, quoted, max_fee),
        );

        Ok(result)
    }

    // ═══════════════════════════════════════════════════════════════════
    // CREDENTIAL ROTATION (called by RecoveryModule after timelock)
    // ═══════════════════════════════════════════════════════════════

    /// Rotate the owner's credentials. This is the final step of social recovery.
    ///
    /// **Security:** This function can only be called by the registered RecoveryModule contract.
    pub fn rotate_credentials(
        env: Env,
        new_credential_id: BytesN<32>,
        new_public_key: BytesN<65>,
    ) -> Result<(), WalletError> {
        let recovery_module = Storage::get_recovery_module(&env)?;
        recovery_module.require_auth();

        signers::rotate_owner(&env, new_credential_id, new_public_key)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CUSTOM ACCOUNT INTERFACE IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════

#[contractimpl]
impl CustomAccountInterface for SmartAccountContract {
    type Error = WalletError;
    type Signature = WalletSignature;

    /// The core authentication function called by the Soroban runtime.
    ///
    /// When this contract's address is used as an authorization source
    /// (e.g., `contract_address.require_auth()`), the runtime calls this
    /// function to verify the transaction is legitimately authorized.
    fn __check_auth(
        env: Env,
        signature_payload: Hash<32>,
        signature: WalletSignature,
        auth_contexts: Vec<Context>,
    ) -> Result<(), WalletError> {
        auth::check_auth(&env, &signature_payload, &signature, &auth_contexts)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod integration_test;
