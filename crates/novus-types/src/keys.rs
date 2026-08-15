use soroban_sdk::{contracttype, Address, BytesN};

/// Storage key enum for the SmartAccount contract.
/// Each variant maps to a specific storage tier (Instance / Persistent / Temporary).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    // ═══ Instance Storage (loaded every call, keep small) ═══
    /// Core wallet configuration
    Config,
    /// Initialization guard
    Initialized,

    // ═══ Persistent Storage (per-key, archivable, restorable) ═══
    /// Maps credential_id → SignerEntry
    Signer(BytesN<32>),
    /// Global monotonic nonce counter
    Nonce,
    /// Daily spending tracker: (token_contract, day_index) → i128 spent
    DailySpend(Address, u32),
    /// Guardian registry: guardian_address → GuardianInfo
    Guardian(Address),
    /// Recovery proposal: proposal_id → RecoveryProposal
    RecoveryProposal(u32),
    /// Next proposal ID counter
    NextProposalId,
    /// Cooldown-after-recovery end ledger
    RecoveryCooldownUntil,

    // ═══ Temporary Storage (auto-expire, cheapest) ═══
    /// Session key config: session_id → SessionConfig
    SessionKey(BytesN<32>),
    /// Recovery timelock: proposal_id → expiry ledger
    RecoveryTimelock(u32),
    /// Replay guard: payload_hash → true (TTL ~24h)
    ReplayGuard(BytesN<32>),
}

/// Storage key enum for the RecoveryModule contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryDataKey {
    /// The SmartAccount this module is bound to
    SmartAccount,
}

/// Storage key enum for the PolicyEngine contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyDataKey {
    /// Admin address
    Admin,
    /// Global whitelist
    WhitelistedContract(Address),
    /// Per-token daily limit override
    TokenDailyLimit(Address),
}

/// Storage key enum for the Paymaster contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaymasterDataKey {
    /// Admin address
    Admin,
    /// Relayer address
    Relayer,
    /// Accepted fee tokens list
    AcceptedTokens,
    /// Fee margin in basis points
    FeeMarginBps,
    /// Wrapped XLM SAC address
    XlmToken,
    /// Fee receipt (temporary): receipt_hash → amount
    FeeReceipt(BytesN<32>),
}
