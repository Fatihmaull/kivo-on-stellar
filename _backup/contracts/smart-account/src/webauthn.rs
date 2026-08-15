/// WebAuthn message reconstruction and secp256r1 signature verification.
///
/// Per the WebAuthn specification, the authenticator signs:
///   SHA-256(authenticator_data || client_data_json_hash)
///
/// where client_data_json_hash = SHA-256(client_data_json),
/// and client_data_json contains the challenge (our signature_payload).
///
/// This module reconstructs that signed message from its components
/// so we can verify against the secp256r1 public key.

use soroban_sdk::{Bytes, BytesN, Env};
use soroban_sdk::crypto::Hash;

use novus_types::WalletSignature;

/// Reconstruct the WebAuthn signed message from authenticator_data and
/// the client_data_json_hash.
///
/// The WebAuthn standard specifies the signed message as:
///   message = authenticator_data || client_data_json_hash
///
/// The authenticator then signs SHA-256(message), but since Soroban's
/// `secp256r1_verify` handles the final hash internally, we pass the
/// raw concatenated message.
///
/// # Arguments
/// * `authenticator_data` - Raw authenticator data bytes from WebAuthn assertion
/// * `client_data_json_hash` - SHA-256 hash of the clientDataJSON
pub fn build_webauthn_message(
    env: &Env,
    signature: &WalletSignature,
) -> Bytes {
    let mut message = Bytes::new(env);
    message.append(&signature.authenticator_data);
    message.append(&Bytes::from_array(env, &signature.client_data_json_hash.to_array()));
    message
}

/// Verify a WebAuthn secp256r1 (P-256) signature.
///
/// This calls the Protocol 21 host function `secp256r1_verify` which
/// performs the verification at native speed (~10x cheaper than EVM).
///
/// # Panics
/// Panics if the signature is invalid (desired behavior in __check_auth).
pub fn verify_passkey_signature(
    env: &Env,
    public_key: &BytesN<65>,
    signature: &WalletSignature,
) {
    let message = build_webauthn_message(env, signature);
    let message_hash = env.crypto().sha256(&message);

    // secp256r1_verify verifies ECDSA signature over SHA-256 message hash
    // Panics on failure — which causes __check_auth to fail as expected.
    env.crypto().secp256r1_verify(
        public_key,
        &message_hash,
        &signature.signature_bytes,
    );
}

/// Verify an ed25519 signature.
///
/// Used for traditional Stellar keypair signers and session keys.
///
/// # Arguments
/// * `public_key_65` - The 65-byte padded key (only first 32 bytes used)
/// * `payload` - The signature payload hash from the Soroban runtime
/// * `signature_bytes` - The 64-byte ed25519 signature
///
/// # Panics
/// Panics if the signature is invalid.
pub fn verify_ed25519_signature(
    env: &Env,
    public_key_65: &BytesN<65>,
    payload: &Hash<32>,
    signature_bytes: &BytesN<64>,
) {
    // Extract the 32-byte ed25519 public key from the padded 65-byte field
    let pk_bytes = public_key_65.to_array();
    let mut ed_key = [0u8; 32];
    ed_key.copy_from_slice(&pk_bytes[..32]);
    let public_key = BytesN::from_array(env, &ed_key);

    let payload_bytes = BytesN::from_array(env, &payload.to_array());

    env.crypto().ed25519_verify(&public_key, &payload_bytes.into(), signature_bytes);
}

/// Pad a 32-byte ed25519 key into the 65-byte SignerEntry.public_key format.
/// Fills the remaining 33 bytes with zeros.
pub fn pad_ed25519_key(env: &Env, key_32: &BytesN<32>) -> BytesN<65> {
    let key_bytes = key_32.to_array();
    let mut padded = [0u8; 65];
    padded[..32].copy_from_slice(&key_bytes);
    BytesN::from_array(env, &padded)
}
