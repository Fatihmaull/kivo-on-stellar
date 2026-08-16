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
    /// The `WalletConfig.credential_epoch` this signer was registered under.
    ///
    /// Bumped by `rotate_owner` (the final step of social recovery). Any
    /// signer whose `epoch` no longer matches the wallet's current epoch is
    /// rejected during `__check_auth` — this invalidates every credential
    /// that existed before a recovery, including ones an attacker may have
    /// silently added, without needing to enumerate and delete them.
    pub epoch: u32,
}

/// Custom signature structure passed to `__check_auth`.
/// The SDK constructs this from the WebAuthn assertion response (Passkey
/// signers) or from a plain keypair signature (Ed25519 / session signers).
///
/// The passkey-only fields are flattened directly into this struct rather
/// than nested behind an `Option<PasskeyAssertion>` — left empty for
/// Ed25519/session signatures — because that's what compiles cleanly
/// through the `#[contracttype]` XDR derive; it also keeps this the one
/// wire shape every caller builds, matching how `CustomAccountInterface`
/// expects a single concrete `Signature` type.
#[contracttype]
#[derive(Clone, Debug)]
pub struct WalletSignature {
    /// The credential ID identifying which signer produced this signature.
    /// Must match a registered SignerEntry, or a live SessionKey.
    pub credential_id: BytesN<32>,
    /// - Ed25519 / SessionKey: the 64-byte standard ed25519 signature.
    /// - Passkey: the 64-byte ECDSA signature (r || s, low-S normalized)
    ///   over SHA-256(authenticator_data || SHA-256(client_data_json)).
    pub signature_bytes: BytesN<64>,
    /// Passkey only — raw authenticator data (rpIdHash || flags || signCount
    /// || ...). Empty for Ed25519/SessionKey.
    pub authenticator_data: Bytes,
    /// Passkey only — the RAW clientDataJSON bytes exactly as returned by
    /// the authenticator (never a caller-supplied hash — the contract
    /// itself locates the base64url challenge inside this JSON and asserts
    /// it equals the transaction's signature payload, which is what binds
    /// the signature to *this* transaction). Empty for Ed25519/SessionKey.
    pub client_data_json: Bytes,
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
    /// SHA-256 hash of the WebAuthn Relying Party ID (origin) this wallet's
    /// passkeys are bound to. Every Passkey-type signature is checked
    /// against this so an assertion collected by any other site cannot be
    /// replayed against this wallet.
    pub rp_id_hash: BytesN<32>,
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
    /// Bumped every time `rotate_owner` runs (final step of social
    /// recovery). Signers registered under an older epoch are rejected.
    pub credential_epoch: u32,
}
