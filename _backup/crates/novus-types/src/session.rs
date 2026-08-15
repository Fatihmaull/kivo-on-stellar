use soroban_sdk::{contracttype, Address, Symbol, Vec};

/// Session key configuration stored in Temporary storage.
/// Auto-expires when the TTL reaches zero — no cleanup needed.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// Contract addresses this session key is allowed to call.
    /// Empty vec means all contracts are allowed (dangerous — not recommended).
    pub allowed_contracts: Vec<Address>,
    /// Function names this session key is allowed to invoke.
    /// Empty vec means all functions are allowed (dangerous — not recommended).
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
}
