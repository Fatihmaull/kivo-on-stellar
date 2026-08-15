use soroban_sdk::{contracttype, Bytes, BytesN};

/// The type of signer registered with the SmartAccount.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignerType {
    /// WebAuthn/Passkey using secp256r1 (P-256) curve.
    /// Primary authentication method — biometric-backed, non-exportable keys.
    Passkey,
    /// Traditional Stellar ed25519 keypair.
    /// Used as backup signer or for programmatic access.
    Ed25519,
    /// Ephemeral session key (ed25519) with scoped permissions.
    /// Auto-expires via Temporary storage TTL.
    SessionKey,
    /// Recovery guardian — cannot authorize normal transactions,
    /// only participates in recovery proposals.
    Guardian,
}

/// A registered signer entry stored in Persistent storage.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SignerEntry {
    /// Type of this signer (determines verification logic)
    pub signer_type: SignerType,
    /// Public key bytes.
    /// - Passkey (secp256r1): 65 bytes uncompressed (0x04 || x || y)
    /// - Ed25519: first 32 bytes used, remaining padded with zeros
    pub public_key: BytesN<65>,
    /// WebAuthn credential ID or derived identifier for non-passkey signers.
    /// Used as the storage lookup key.
    pub credential_id: BytesN<32>,
    /// Ledger timestamp when this signer was registered
    pub added_at: u64,
    /// Signature weight for potential multi-sig scenarios.
    /// Default: 10 for Passkey/Ed25519, 1 for SessionKey, 0 for Guardian.
    pub weight: u32,
}

/// Custom signature structure passed to `__check_auth`.
/// The SDK constructs this from the WebAuthn assertion response.
#[contracttype]
#[derive(Clone, Debug)]
pub struct WalletSignature {
    /// The credential ID identifying which signer produced this signature.
    /// Must match a registered SignerEntry.
    pub credential_id: BytesN<32>,
    /// The raw signature bytes.
    /// - Passkey: 64 bytes (r || s), DER-decoded by the SDK
    /// - Ed25519: 64 bytes standard ed25519 signature
    pub signature_bytes: BytesN<64>,
    /// WebAuthn authenticator data (for Passkey signers).
    /// Contains rpIdHash, flags, and sign counter.
    /// Empty/unused for Ed25519 and SessionKey signers.
    pub authenticator_data: Bytes,
    /// Client data JSON hash (for Passkey signers).
    /// The SHA-256 hash of the client data JSON produced by navigator.credentials.get().
    /// Empty/unused for Ed25519 and SessionKey signers.
    pub client_data_json_hash: BytesN<32>,
    /// Sequential nonce for replay protection.
    /// Must match the current stored nonce value.
    pub nonce: u128,
}

/// Configuration for the wallet, stored in Instance storage.
#[contracttype]
#[derive(Clone, Debug)]
pub struct WalletConfig {
    /// Contract version for upgrade tracking
    pub version: u32,
    /// Credential ID of the primary owner passkey
    pub owner_credential_id: BytesN<32>,
    /// Number of guardian approvals required for recovery (m in m-of-n)
    pub recovery_threshold: u32,
    /// Timelock delay in ledger count before recovery executes (~5s per ledger).
    /// Default: 34_560 (~48 hours)
    pub recovery_timelock_ledgers: u32,
    /// Total number of registered guardians
    pub guardian_count: u32,
    /// Default daily spending limit in token base units.
    /// Applied per-token unless overridden by PolicyEngine.
    pub default_daily_limit: i128,
}
