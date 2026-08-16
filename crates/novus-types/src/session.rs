use soroban_sdk::{contracttype, Address, BytesN, Symbol, Vec};

/// Session key configuration stored in Temporary storage.
/// Auto-expires when the TTL reaches zero — no cleanup needed.
///
/// This is the *only* record a session key needs. Earlier revisions also
/// registered a Persistent `SignerEntry` for the session's credential_id;
/// that meant every expired session left a permanently-billed, permanently
/// useless Persistent entry behind. Carrying the public key here means
/// nothing about a session outlives its own TTL.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// The session's ed25519 public key (32 bytes).
    pub public_key: BytesN<32>,
    /// Contract addresses this session key is allowed to call.
    /// Must be non-empty — an empty list denies every target (fail-closed).
    pub allowed_contracts: Vec<Address>,
    /// Function names this session key is allowed to invoke.
    /// Must be non-empty — an empty list denies every function (fail-closed).
    pub allowed_functions: Vec<Symbol>,
    /// Maximum token amount allowed per individual transaction
    pub max_amount_per_tx: i128,
    /// Maximum cumulative token amount for the entire session lifetime
    pub max_total_amount: i128,
    /// Running total of amounts spent through this session key
    pub spent_amount: i128,
    /// Ledger sequence number at which this session expires.
    /// After this ledger, the Temporary storage entry is auto-deleted.
    pub expires_at_ledger: u32,
    /// The `WalletConfig.credential_epoch` this session was created under.
    /// Invalidated the same way a Persistent signer is — see
    /// `SignerEntry.epoch`.
    pub epoch: u32,
}
