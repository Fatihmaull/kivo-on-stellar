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
    Address, BytesN, Env, Vec,
};

use novus_types::{
    GuardianInfo, SessionConfig, SignerEntry, SignerType, WalletConfig, WalletError,
    WalletSignature,
};

use crate::storage::Storage;

/// ═══════════════════════════════════════════════════════════════════════
/// NovusWallet SmartAccount Contract
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
/// - Policy enforcement (spending limits, whitelists)
/// - Session key management for dApp interactions
/// - Guardian management for social recovery
#[contract]
pub struct SmartAccountContract;

#[contractimpl]
impl SmartAccountContract {
    // ═══════════════════════════════════════════════════════════════════
    // INITIALIZATION
    // ═══════════════════════════════════════════════════════════════════

    /// Initialize a new SmartAccount with the owner's first passkey.
    ///
    /// # Arguments
    /// * `owner_credential_id` - WebAuthn credential ID from passkey registration
    /// * `owner_public_key` - 65-byte uncompressed secp256r1 public key (0x04 || x || y)
    /// * `recovery_threshold` - Number of guardian approvals needed for recovery (m)
    /// * `recovery_timelock_ledgers` - Delay in ledgers before recovery executes
    /// * `default_daily_limit` - Default per-token daily spending limit (in base units)
    pub fn initialize(
        env: Env,
        owner_credential_id: BytesN<32>,
        owner_public_key: BytesN<65>,
        recovery_threshold: u32,
        recovery_timelock_ledgers: u32,
        default_daily_limit: i128,
    ) -> Result<(), WalletError> {
        // Guard: prevent re-initialization
        if Storage::is_initialized(&env) {
            return Err(WalletError::AlreadyInitialized);
        }

        // Store the wallet configuration in Instance storage
        let config = WalletConfig {
            version: 1,
            owner_credential_id: owner_credential_id.clone(),
            recovery_threshold,
            recovery_timelock_ledgers,
            guardian_count: 0,
            default_daily_limit,
        };
        Storage::set_config(&env, &config);

        // Register the owner's passkey as the first signer
        let owner_signer = SignerEntry {
            signer_type: SignerType::Passkey,
            public_key: owner_public_key,
            credential_id: owner_credential_id,
            added_at: env.ledger().timestamp(),
            weight: 10, // Highest weight for owner
        };
        Storage::set_signer(&env, &owner_signer);

        // Initialize the nonce counter
        Storage::set_nonce(&env, 0);

        // Mark as initialized
        Storage::set_initialized(&env, true);

        // Extend Instance storage TTL generously on init
        env.storage()
            .instance()
            .extend_ttl(103_680, 103_680); // ~6 days

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // PUBLIC READ METHODS
    // ═══════════════════════════════════════════════════════════════════

    /// Get the current wallet configuration.
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

    /// Check if a credential ID is a registered signer.
    pub fn is_signer(env: Env, credential_id: BytesN<32>) -> bool {
        Storage::has_signer(&env, &credential_id)
    }

    /// Get signer details by credential ID.
    pub fn get_signer(env: Env, credential_id: BytesN<32>) -> Result<SignerEntry, WalletError> {
        Storage::get_signer(&env, &credential_id)
    }

    /// Check if an address is a registered guardian.
    pub fn is_guardian(env: Env, address: Address) -> bool {
        Storage::has_guardian(&env, &address)
    }

    // ═══════════════════════════════════════════════════════════════════
    // SIGNER MANAGEMENT (requires owner auth)
    // ═══════════════════════════════════════════════════════════════════

    /// Register a new signer (passkey, ed25519, or guardian).
    /// Requires authentication from the contract itself (triggers __check_auth).
    pub fn add_signer(
        env: Env,
        signer_entry: SignerEntry,
    ) -> Result<(), WalletError> {
        // Authenticate: the caller must be this contract (owner auth)
        env.current_contract_address().require_auth();

        signers::add_signer(&env, signer_entry)
    }

    /// Remove a signer by credential ID. Cannot remove the last passkey signer.
    pub fn remove_signer(
        env: Env,
        credential_id: BytesN<32>,
    ) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();

        signers::remove_signer(&env, &credential_id)
    }

    // ═══════════════════════════════════════════════════════════════════
    // GUARDIAN MANAGEMENT (requires owner auth)
    // ═══════════════════════════════════════════════════════════════════

    /// Add a guardian for social recovery.
    pub fn add_guardian(
        env: Env,
        guardian: GuardianInfo,
    ) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();

        signers::add_guardian(&env, guardian)
    }

    /// Remove a guardian.
    pub fn remove_guardian(
        env: Env,
        guardian_address: Address,
    ) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();

        signers::remove_guardian(&env, &guardian_address)
    }

    // ═══════════════════════════════════════════════════════════════════
    // SESSION KEY MANAGEMENT
    // ═══════════════════════════════════════════════════════════════════

    /// Create a new ephemeral session key with scoped permissions.
    /// Returns the session ID (derived from the session public key).
    pub fn create_session(
        env: Env,
        session_public_key: BytesN<32>,
        config: SessionConfig,
    ) -> Result<BytesN<32>, WalletError> {
        env.current_contract_address().require_auth();

        session::create_session(&env, session_public_key, config)
    }

    /// Revoke an active session key immediately.
    pub fn revoke_session(
        env: Env,
        session_id: BytesN<32>,
    ) -> Result<(), WalletError> {
        env.current_contract_address().require_auth();

        session::revoke_session(&env, &session_id)
    }

    // ═══════════════════════════════════════════════════════════════════
    // CREDENTIAL ROTATION (called by RecoveryModule after timelock)
    // ═══════════════════════════════════════════════════════════════════

    /// Rotate the owner's credentials. This is the final step of social recovery.
    ///
    /// **Security:** This function can only be called by the contract itself.
    /// The RecoveryModule triggers this via a cross-contract call that requires
    /// guardian-threshold authorization.
    pub fn rotate_credentials(
        env: Env,
        new_credential_id: BytesN<32>,
        new_public_key: BytesN<65>,
    ) -> Result<(), WalletError> {
        // Only this contract can rotate its own credentials
        env.current_contract_address().require_auth();

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
    ///
    /// # Flow
    /// 1. Verify & increment nonce (replay protection)
    /// 2. Set payload hash replay guard (double protection)
    /// 3. Look up the signer by credential ID
    /// 4. Verify the cryptographic signature (secp256r1 or ed25519)
    /// 5. For session keys: validate scope constraints
    /// 6. Enforce spending policies
    /// 7. Extend TTL on critical storage entries (piggyback refresh)
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
