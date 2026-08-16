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
    /// Address of the registered RecoveryModule contract
    RecoveryModule,
    /// Address of the registered PolicyEngine contract (optional — if unset,
    /// the built-in daily-limit check in `policy.rs` is used instead).
    PolicyEngine,
    /// Address of the registered Paymaster contract (optional — required
    /// only for `execute_sponsored`).
    Paymaster,

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

    // ═══ Temporary Storage (auto-expire, cheapest) ═══
    /// Session key config: session_id → SessionConfig
    SessionKey(BytesN<32>),
    /// Replay guard: payload_hash → true (TTL ~24h)
    ReplayGuard(BytesN<32>),
}

/// Storage key enum for the RecoveryModule contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryDataKey {
    /// The SmartAccount this module is bound to
    SmartAccount,
    /// Maps proposal_id -> RecoveryProposal (Persistent — the timelock
    /// expiry now lives inside the proposal itself; see `RecoveryProposal`)
    Proposal(u32),
    /// Counter for next proposal_id (Persistent)
    NextProposalId,
}

/// Storage key enum for the PolicyEngine contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyDataKey {
    /// Admin address
    Admin,
    /// Whether the global whitelist is actively enforced. While `false`,
    /// `check_policy` only enforces per-token limit overrides — this lets a
    /// wallet adopt the PolicyEngine before curating a whitelist without
    /// locking itself out.
    WhitelistEnforced,
    /// Global whitelist membership
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
    /// Fee margin in basis points, applied on top of the relayer's quote by
    /// callers when they compute what to charge (informational — the
    /// contract itself does not apply it automatically).
    FeeMarginBps,
    /// Wrapped XLM SAC address
    XlmToken,
}
