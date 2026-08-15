use soroban_sdk::{contracttype, Address, BytesN, Symbol, Vec};

/// Status of a recovery proposal through its lifecycle.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    /// Proposal created, waiting for guardian approvals
    Pending,
    /// Threshold met, timelock countdown started
    TimelockStarted,
    /// Recovery executed — credentials rotated
    Executed,
    /// Cancelled by the legitimate owner during timelock
    Cancelled,
}

/// A social recovery proposal stored in Persistent storage.
#[contracttype]
#[derive(Clone, Debug)]
pub struct RecoveryProposal {
    /// Unique proposal identifier (monotonically increasing)
    pub id: u32,
    /// The new passkey credential ID to replace the current owner's
    pub new_credential_id: BytesN<32>,
    /// The new passkey public key (65 bytes uncompressed secp256r1)
    pub new_public_key: BytesN<65>,
    /// List of guardian addresses that have approved this proposal
    pub approvals: Vec<Address>,
    /// Ledger sequence when the proposal was created
    pub created_at_ledger: u32,
    /// Current lifecycle status
    pub status: ProposalStatus,
}

/// Guardian information stored in Persistent storage.
#[contracttype]
#[derive(Clone, Debug)]
pub struct GuardianInfo {
    /// Guardian's Stellar address (can be a contract or classic account)
    pub address: Address,
    /// Ledger timestamp when this guardian was added
    pub added_at: u64,
    /// Human-readable label for the guardian (e.g., "alice_phone", "hardware_key_1")
    pub alias: Symbol,
}
