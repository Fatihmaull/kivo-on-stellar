/// WebAuthn assertion verification.
///
/// Per the WebAuthn spec, an authenticator signs:
///   SHA-256(authenticator_data || SHA-256(client_data_json))
///
/// `client_data_json` is a JSON document containing (among other fields)
/// the base64url-encoded challenge the caller asked the authenticator to
/// sign. The contract is handed `signature_payload` — the transaction hash
/// — as that challenge, and its entire job is to confirm that the
/// authenticator actually signed *this* challenge, embedded in *this*
/// clientDataJSON, over *this* authenticator_data. Skipping any one of
/// those checks turns "a passkey signed something once" into "a passkey
/// authorized this specific transaction," which is the only claim that
/// matters.
use soroban_sdk::{crypto::Hash, Bytes, BytesN, Env};

use novus_types::WalletError;

/// `"challenge":"` — the tag immediately preceding the challenge value in
/// clientDataJSON, per the WebAuthn `CollectedClientData` serialization.
const CHALLENGE_TAG: &[u8] = b"\"challenge\":\"";
/// Present in clientDataJSON for a `navigator.credentials.get()` assertion.
/// Rejecting its absence stops a `webauthn.create` (registration) ceremony
/// from being replayed as an authorization.
const TYPE_GET_TAG: &[u8] = b"\"type\":\"webauthn.get\"";

/// Base64url (RFC 4648 §5, no padding) alphabet.
const B64URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encode exactly 32 bytes as base64url with no padding → 43 ASCII chars.
/// This is the fixed output length for a SHA-256 digest, which is what
/// `signature_payload` always is.
pub(crate) fn base64url_encode_32(input: &[u8; 32]) -> [u8; 43] {
    let mut out = [0u8; 43];
    let mut oi = 0usize;
    let mut i = 0usize;

    // Full 3-byte -> 4-char groups (10 groups = 30 bytes)
    while i + 3 <= 32 {
        let b0 = input[i] as u32;
        let b1 = input[i + 1] as u32;
        let b2 = input[i + 2] as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out[oi] = B64URL_ALPHABET[((n >> 18) & 0x3F) as usize];
        out[oi + 1] = B64URL_ALPHABET[((n >> 12) & 0x3F) as usize];
        out[oi + 2] = B64URL_ALPHABET[((n >> 6) & 0x3F) as usize];
        out[oi + 3] = B64URL_ALPHABET[(n & 0x3F) as usize];
        oi += 4;
        i += 3;
    }

    // Trailing 2 bytes -> 3 chars (32 = 10*3 + 2, so this always fires once)
    let remaining = 32 - i;
    if remaining == 2 {
        let b0 = input[i] as u32;
        let b1 = input[i + 1] as u32;
        let n = (b0 << 16) | (b1 << 8);
        out[oi] = B64URL_ALPHABET[((n >> 18) & 0x3F) as usize];
        out[oi + 1] = B64URL_ALPHABET[((n >> 12) & 0x3F) as usize];
        out[oi + 2] = B64URL_ALPHABET[((n >> 6) & 0x3F) as usize];
    } else if remaining == 1 {
        let b0 = input[i] as u32;
        let n = b0 << 16;
        out[oi] = B64URL_ALPHABET[((n >> 18) & 0x3F) as usize];
        out[oi + 1] = B64URL_ALPHABET[((n >> 12) & 0x3F) as usize];
    }

    out
}

/// Find the first occurrence of `needle` in `haystack`, returning the
/// starting index. `haystack` is bounded by clientDataJSON's real-world
/// size (well under a kilobyte), so a naive scan is cheap.
fn find_subsequence(haystack: &Bytes, needle: &[u8]) -> Option<u32> {
    let h_len = haystack.len();
    let n_len = needle.len() as u32;
    if n_len == 0 || n_len > h_len {
        return None;
    }
    let mut i: u32 = 0;
    while i + n_len <= h_len {
        let mut matched = true;
        let mut j: u32 = 0;
        while j < n_len {
            if haystack.get(i + j) != Some(needle[j as usize]) {
                matched = false;
                break;
            }
            j += 1;
        }
        if matched {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Verify a WebAuthn passkey assertion authorizes `payload`.
///
/// # Checks, in order
/// 1. **Challenge binding** — base64url(payload) appears immediately after
///    `"challenge":"` in the raw clientDataJSON.
/// 2. **Ceremony type** — clientDataJSON declares `"type":"webauthn.get"`.
/// 3. **Relying Party** — authenticator_data's rpIdHash matches the
///    wallet's configured origin.
/// 4. **User Present / User Verified flags** — a human actually touched the
///    authenticator and the biometric/PIN gate ran.
/// 5. **Signature** — secp256r1 over SHA-256(authData || SHA-256(clientDataJSON)).
///
/// # Errors
/// Returns a specific `WalletError` for whichever check fails first, and
/// panics (via the host's `secp256r1_verify`) if the signature itself is
/// invalid — either way `__check_auth` aborts.
pub fn verify_passkey(
    env: &Env,
    public_key: &BytesN<65>,
    rp_id_hash: &BytesN<32>,
    payload: &Hash<32>,
    authenticator_data: &Bytes,
    client_data_json: &Bytes,
    signature: &BytesN<64>,
) -> Result<(), WalletError> {
    // ── 1: Challenge binding ──
    let expected_challenge = base64url_encode_32(&payload.to_array());
    let tag_at =
        find_subsequence(client_data_json, CHALLENGE_TAG).ok_or(WalletError::MalformedClientData)?;
    let challenge_start = tag_at + CHALLENGE_TAG.len() as u32;
    let json_len = client_data_json.len();
    if challenge_start + 43 > json_len {
        return Err(WalletError::MalformedClientData);
    }
    let actual_challenge = client_data_json.slice(challenge_start..challenge_start + 43);
    let expected_bytes = Bytes::from_array(env, &expected_challenge);
    if actual_challenge != expected_bytes {
        return Err(WalletError::ChallengeMismatch);
    }

    // ── 2: Ceremony type ──
    if find_subsequence(client_data_json, TYPE_GET_TAG).is_none() {
        return Err(WalletError::WrongCeremonyType);
    }

    // ── 3: Relying Party ──
    if authenticator_data.len() < 37 {
        return Err(WalletError::MalformedAssertion);
    }
    let actual_rp_id_hash = authenticator_data.slice(0..32);
    let expected_rp_id_hash = Bytes::from_array(env, &rp_id_hash.to_array());
    if actual_rp_id_hash != expected_rp_id_hash {
        return Err(WalletError::RpIdMismatch);
    }

    // ── 4: Flags byte (offset 32): bit 0 = UP, bit 2 = UV ──
    let flags = authenticator_data
        .get(32)
        .ok_or(WalletError::MalformedAssertion)?;
    if flags & 0x01 == 0 {
        return Err(WalletError::UserPresenceMissing);
    }
    if flags & 0x04 == 0 {
        return Err(WalletError::UserVerificationMissing);
    }

    // ── 5: Signature ──
    // message = authenticator_data || SHA-256(client_data_json)
    let client_data_hash = env.crypto().sha256(client_data_json);
    let mut message = Bytes::new(env);
    message.append(authenticator_data);
    message.append(&Bytes::from_array(env, &client_data_hash.to_array()));
    let message_hash = env.crypto().sha256(&message);

    // Panics on an invalid signature — this is what aborts __check_auth.
    env.crypto().secp256r1_verify(public_key, &message_hash, signature);

    Ok(())
}

/// Verify an ed25519 signature.
///
/// Used for traditional Stellar keypair signers and session keys. The
/// payload IS the message — no reconstruction needed, since ed25519
/// signers sign the Soroban authorization payload hash directly.
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
    let pk_bytes = public_key_65.to_array();
    let mut ed_key = [0u8; 32];
    ed_key.copy_from_slice(&pk_bytes[..32]);
    let public_key = BytesN::from_array(env, &ed_key);

    let payload_bytes = BytesN::from_array(env, &payload.to_array());

    env.crypto()
        .ed25519_verify(&public_key, &payload_bytes.into(), signature_bytes);
}

/// Verify an ed25519 signature from a plain 32-byte session key.
pub fn verify_ed25519_session_signature(
    env: &Env,
    public_key_32: &BytesN<32>,
    payload: &Hash<32>,
    signature_bytes: &BytesN<64>,
) {
    let payload_bytes = BytesN::from_array(env, &payload.to_array());
    env.crypto()
        .ed25519_verify(public_key_32, &payload_bytes.into(), signature_bytes);
}

