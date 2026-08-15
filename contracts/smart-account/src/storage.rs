/// Storage abstraction layer for the SmartAccount contract.
///
/// Provides typed read/write access to all three storage tiers
/// (Instance, Persistent, Temporary) with proper TTL management.

use soroban_sdk::{Address, BytesN, Env};

use novus_types::{
    DataKey, GuardianInfo, SessionConfig, SignerEntry, WalletConfig, WalletError,
};

/// TTL constants (in ledger count, ~5s per ledger)
const DAY_IN_LEDGERS: u32 = 17_280;       // ~24 hours
const WEEK_IN_LEDGERS: u32 = 120_960;     // ~7 days
const MONTH_IN_LEDGERS: u32 = 518_400;    // ~30 days

/// Minimum TTL before we extend (threshold to trigger piggyback refresh)
const PERSISTENT_TTL_THRESHOLD: u32 = WEEK_IN_LEDGERS;
/// Target TTL after extension
const PERSISTENT_TTL_EXTEND: u32 = MONTH_IN_LEDGERS;
/// Instance TTL (refreshed on every __check_auth call)
const INSTANCE_TTL_EXTEND: u32 = WEEK_IN_LEDGERS;
/// Replay guard TTL
const REPLAY_GUARD_TTL: u32 = DAY_IN_LEDGERS;

pub struct Storage;

impl Storage {
    // ═══════════════════════════════════════════════════════════════
    // INSTANCE STORAGE (config, initialization flag)
    // ═══════════════════════════════════════════════════════════════

    pub fn is_initialized(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Initialized)
            .unwrap_or(false)
    }

    pub fn set_initialized(env: &Env, initialized: bool) {
        env.storage()
            .instance()
            .set(&DataKey::Initialized, &initialized);
    }

    pub fn get_config(env: &Env) -> Result<WalletConfig, WalletError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(WalletError::NotInitialized)
    }

    pub fn set_config(env: &Env, config: &WalletConfig) {
        env.storage().instance().set(&DataKey::Config, config);
    }

    pub fn get_recovery_module(env: &Env) -> Result<Address, WalletError> {
        env.storage()
            .instance()
            .get(&DataKey::RecoveryModule)
            .ok_or(WalletError::NotInitialized)
    }

    pub fn set_recovery_module(env: &Env, address: &Address) {
        env.storage()
            .instance()
            .set(&DataKey::RecoveryModule, address);
    }

    /// Extend Instance TTL — called during every __check_auth for piggyback refresh.
    pub fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_EXTEND, INSTANCE_TTL_EXTEND);
    }

    // ═══════════════════════════════════════════════════════════════
    // PERSISTENT STORAGE — Signers
    // ═══════════════════════════════════════════════════════════════

    pub fn get_signer(env: &Env, credential_id: &BytesN<32>) -> Result<SignerEntry, WalletError> {
        env.storage()
            .persistent()
            .get(&DataKey::Signer(credential_id.clone()))
            .ok_or(WalletError::SignerNotFound)
    }

    pub fn has_signer(env: &Env, credential_id: &BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Signer(credential_id.clone()))
    }

    pub fn set_signer(env: &Env, signer: &SignerEntry) {
        let key = DataKey::Signer(signer.credential_id.clone());
        env.storage().persistent().set(&key, signer);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND);
    }

    pub fn remove_signer(env: &Env, credential_id: &BytesN<32>) {
        env.storage()
            .persistent()
            .remove(&DataKey::Signer(credential_id.clone()));
    }

    /// Piggyback TTL refresh for a signer entry during __check_auth.
    pub fn bump_signer_ttl(env: &Env, credential_id: &BytesN<32>) {
        let key = DataKey::Signer(credential_id.clone());
        if env.storage().persistent().has(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND);
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // PERSISTENT STORAGE — Nonce
    // ═══════════════════════════════════════════════════════════════

    pub fn get_nonce(env: &Env) -> u128 {
        env.storage()
            .persistent()
            .get(&DataKey::Nonce)
            .unwrap_or(0)
    }

    pub fn set_nonce(env: &Env, nonce: u128) {
        env.storage().persistent().set(&DataKey::Nonce, &nonce);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Nonce, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND);
    }

    // ═══════════════════════════════════════════════════════════════
    // PERSISTENT STORAGE — Daily Spending
    // ═══════════════════════════════════════════════════════════════

    pub fn get_daily_spend(env: &Env, token: &Address, day_index: u32) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::DailySpend(token.clone(), day_index))
            .unwrap_or(0)
    }

    pub fn set_daily_spend(env: &Env, token: &Address, day_index: u32, amount: i128) {
        let key = DataKey::DailySpend(token.clone(), day_index);
        env.storage().persistent().set(&key, &amount);
        // Daily spend entries only need to survive the day + buffer
        env.storage()
            .persistent()
            .extend_ttl(&key, DAY_IN_LEDGERS, DAY_IN_LEDGERS * 2);
    }

    // ═══════════════════════════════════════════════════════════════
    // PERSISTENT STORAGE — Guardians
    // ═══════════════════════════════════════════════════════════════

    #[allow(dead_code)]
    pub fn get_guardian(env: &Env, address: &Address) -> Result<GuardianInfo, WalletError> {
        env.storage()
            .persistent()
            .get(&DataKey::Guardian(address.clone()))
            .ok_or(WalletError::GuardianNotFound)
    }

    pub fn has_guardian(env: &Env, address: &Address) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Guardian(address.clone()))
    }

    pub fn set_guardian(env: &Env, guardian: &GuardianInfo) {
        let key = DataKey::Guardian(guardian.address.clone());
        env.storage().persistent().set(&key, guardian);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND);
    }

    pub fn remove_guardian(env: &Env, address: &Address) {
        env.storage()
            .persistent()
            .remove(&DataKey::Guardian(address.clone()));
    }

    // ═══════════════════════════════════════════════════════════════
    // TEMPORARY STORAGE — Session Keys
    // ═══════════════════════════════════════════════════════════════

    pub fn get_session_config(
        env: &Env,
        session_id: &BytesN<32>,
    ) -> Result<SessionConfig, WalletError> {
        env.storage()
            .temporary()
            .get(&DataKey::SessionKey(session_id.clone()))
            .ok_or(WalletError::SessionKeyExpired)
    }

    pub fn set_session_config(
        env: &Env,
        session_id: &BytesN<32>,
        config: &SessionConfig,
        ttl_ledgers: u32,
    ) {
        let key = DataKey::SessionKey(session_id.clone());
        env.storage().temporary().set(&key, config);
        env.storage()
            .temporary()
            .extend_ttl(&key, ttl_ledgers, ttl_ledgers);
    }

    pub fn remove_session(env: &Env, session_id: &BytesN<32>) {
        env.storage()
            .temporary()
            .remove(&DataKey::SessionKey(session_id.clone()));
    }

    // ═══════════════════════════════════════════════════════════════
    // TEMPORARY STORAGE — Replay Guard
    // ═══════════════════════════════════════════════════════════════

    pub fn has_replay_guard(env: &Env, payload_hash: &BytesN<32>) -> bool {
        env.storage()
            .temporary()
            .has(&DataKey::ReplayGuard(payload_hash.clone()))
    }

    pub fn set_replay_guard(env: &Env, payload_hash: &BytesN<32>) {
        let key = DataKey::ReplayGuard(payload_hash.clone());
        env.storage().temporary().set(&key, &true);
        env.storage()
            .temporary()
            .extend_ttl(&key, REPLAY_GUARD_TTL, REPLAY_GUARD_TTL);
    }
}
