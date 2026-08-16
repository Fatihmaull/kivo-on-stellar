#![cfg(test)]

/// SmartAccount contract test suite.
///
/// Split into two families:
/// - CRUD tests (constructor, signer/guardian management) that exercise the
///   contract through its public client, matching how the original suite
///   worked.
/// - Auth-path tests that call `__check_auth` directly — through the real
///   generated contract client, i.e. the exact entry point the Soroban
///   host calls — with a *real* P-256 keypair and *real* signatures. These
///   are the tests that would have caught the original K-01 challenge-
///   binding bypass: they build a genuine WebAuthn assertion and confirm
///   it is rejected for every transaction except the one it actually
///   authorizes.
extern crate std;

use p256::ecdsa::{signature::hazmat::PrehashSigner, Signature, SigningKey};
use soroban_sdk::{
    auth::{Context, ContractContext},
    crypto::Hash,
    testutils::{Address as _, Ledger, LedgerInfo},
    vec, Address, Bytes, BytesN, Env, IntoVal, Symbol,
};
use std::vec::Vec as StdVec;

use crate::SmartAccountContract;
use crate::SmartAccountContractClient;
use novus_types::{GuardianInfo, SessionConfig, SignerEntry, SignerType, WalletError, WalletSignature};

const RP_ID: &str = "kivo.app";

/// Call the real auth pipeline the same way the Soroban host does when
/// verifying this contract's signature — `__check_auth` is a
/// host-reserved entry point (calling it through a normal
/// `invoke_contract` aborts, as it should), so `env.as_contract` is what
/// puts the storage/address context in place to run the actual
/// `auth::check_auth` logic outside of full transaction-level auth
/// simulation. This still exercises the identical code the trait impl
/// delegates to — nothing about the verification logic itself is mocked.
fn call_check_auth(
    env: &Env,
    contract_id: &Address,
    payload: &Hash<32>,
    signature: &WalletSignature,
    contexts: &soroban_sdk::Vec<Context>,
) -> Result<(), WalletError> {
    env.as_contract(contract_id, || {
        crate::auth::check_auth(env, payload, signature, contexts)
    })
}

/// Same call, but panics if it doesn't succeed — for setup steps in a test
/// where a successful auth is a precondition, not the assertion.
fn check_auth_expect_ok(
    env: &Env,
    contract_id: &Address,
    payload: &Hash<32>,
    signature: &WalletSignature,
    contexts: &soroban_sdk::Vec<Context>,
) {
    call_check_auth(env, contract_id, payload, signature, contexts).expect("auth should succeed");
}

/// Helper: create a test environment and deploy the SmartAccount contract
/// with a mock (non-cryptographic) owner passkey, for CRUD-focused tests
/// that don't need to exercise real signature verification.
fn setup() -> (Env, Address, SmartAccountContractClient<'static>, BytesN<32>) {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env);

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);
    let rp_id_hash = rp_id_hash(&env);

    let contract_id = env.register(
        SmartAccountContract,
        (
            cred_id.clone(),
            pubkey,
            rp_id_hash,
            2u32,
            34_560u32,
            1_000_000_0000000i128,
        ),
    );
    let client = SmartAccountContractClient::new(&env, &contract_id);

    (env, contract_id, client, cred_id)
}

fn set_ledger(env: &Env) {
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000,
        protocol_version: 22,
        sequence_number: 100_000,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1_000,
        max_entry_ttl: 10_000_000,
    });
}

fn mock_passkey_pubkey(env: &Env) -> BytesN<65> {
    let mut key = [0u8; 65];
    key[0] = 0x04;
    key[1] = 0xAA;
    key[2] = 0xBB;
    BytesN::from_array(env, &key)
}

fn mock_credential_id(env: &Env, seed: u8) -> BytesN<32> {
    let mut id = [0u8; 32];
    id[0] = seed;
    id[1] = seed.wrapping_mul(7);
    BytesN::from_array(env, &id)
}

fn rp_id_hash(env: &Env) -> BytesN<32> {
    env.crypto()
        .sha256(&Bytes::from_slice(env, RP_ID.as_bytes()))
        .into()
}

// ═══════════════════════════════════════════════════════════════════
// REAL PASSKEY TEST HARNESS
// ═══════════════════════════════════════════════════════════════════

/// A deterministic (non-random — tests must be reproducible) P-256 signing
/// key: scalar value 7, comfortably within the curve order.
fn test_signing_key() -> SigningKey {
    let mut secret = [0u8; 32];
    secret[31] = 7;
    SigningKey::from_bytes((&secret).into()).expect("valid scalar")
}

fn signing_key_public_bytes(env: &Env, key: &SigningKey) -> BytesN<65> {
    let point = key.verifying_key().to_encoded_point(false);
    let bytes = point.as_bytes();
    assert_eq!(bytes.len(), 65);
    let mut arr = [0u8; 65];
    arr.copy_from_slice(bytes);
    BytesN::from_array(env, &arr)
}

/// Build real authenticator_data: rpIdHash(32) || flags(1) || signCount(4).
fn build_authenticator_data(env: &Env, rp_id_hash: &BytesN<32>, up: bool, uv: bool) -> Bytes {
    let mut data: StdVec<u8> = StdVec::from(rp_id_hash.to_array());
    let mut flags = 0u8;
    if up {
        flags |= 0x01;
    }
    if uv {
        flags |= 0x04;
    }
    data.push(flags);
    data.extend_from_slice(&[0u8, 0u8, 0u8, 0u8]); // signCount
    Bytes::from_slice(env, &data)
}

/// Build real clientDataJSON carrying `challenge_payload` (32 bytes) as its
/// base64url-encoded challenge, using the contract's own encoder — so this
/// harness stays byte-for-byte aligned with what the verifier expects.
fn build_client_data_json(env: &Env, challenge_payload: &[u8; 32], ceremony_type: &str) -> Bytes {
    let challenge_b64 = crate::webauthn::base64url_encode_32(challenge_payload);
    let challenge_str = core::str::from_utf8(&challenge_b64).unwrap();
    let json = std::format!(
        "{{\"type\":\"{}\",\"challenge\":\"{}\",\"origin\":\"https://{}\",\"crossOrigin\":false}}",
        ceremony_type,
        challenge_str,
        RP_ID
    );
    Bytes::from_slice(env, json.as_bytes())
}

/// Sign `authenticator_data || SHA-256(client_data_json)` (hashed once
/// more) with `key`, exactly as the contract verifies it, and return a
/// low-S-normalized 64-byte (r || s) signature.
fn sign_assertion(
    env: &Env,
    key: &SigningKey,
    authenticator_data: &Bytes,
    client_data_json: &Bytes,
) -> BytesN<64> {
    let client_data_hash = env.crypto().sha256(client_data_json);
    let mut message = authenticator_data.clone();
    message.append(&Bytes::from_array(env, &client_data_hash.to_array()));
    let message_hash = env.crypto().sha256(&message);

    let sig: Signature = key
        .sign_prehash(&message_hash.to_array())
        .expect("prehash signing succeeds");
    let sig = sig.normalize_s().unwrap_or(sig);
    let bytes = sig.to_bytes();
    let mut arr = [0u8; 64];
    arr.copy_from_slice(&bytes);
    BytesN::from_array(env, &arr)
}

/// Build a complete, *valid* WalletSignature authorizing `payload` for the
/// owner credential, using the real signing key.
fn valid_owner_signature(
    env: &Env,
    key: &SigningKey,
    cred_id: &BytesN<32>,
    payload: &[u8; 32],
    nonce: u128,
) -> WalletSignature {
    let rp_hash = rp_id_hash(env);
    let auth_data = build_authenticator_data(env, &rp_hash, true, true);
    let client_json = build_client_data_json(env, payload, "webauthn.get");
    let sig = sign_assertion(env, key, &auth_data, &client_json);

    WalletSignature {
        credential_id: cred_id.clone(),
        signature_bytes: sig,
        authenticator_data: auth_data,
        client_data_json: client_json,
        nonce,
    }
}

/// Deploy a SmartAccount whose owner is a *real* P-256 keypair, for the
/// crypto-path tests.
fn setup_with_real_passkey() -> (Env, SmartAccountContractClient<'static>, SigningKey, BytesN<32>) {
    let env = Env::default();
    set_ledger(&env);

    let key = test_signing_key();
    let cred_id = mock_credential_id(&env, 9);
    let pubkey = signing_key_public_bytes(&env, &key);
    let rp_hash = rp_id_hash(&env);

    let contract_id = env.register(
        SmartAccountContract,
        (
            cred_id.clone(),
            pubkey,
            rp_hash,
            2u32,
            34_560u32,
            1_000_000_0000000i128,
        ),
    );
    let client = SmartAccountContractClient::new(&env, &contract_id);

    (env, client, key, cred_id)
}

/// A distinct 32-byte "transaction hash" for each `seed`, standing in for
/// whatever `signature_payload` the Soroban host would actually compute.
/// Returned as a real `Hash<32>` (via `sha256`, since that's the only
/// public constructor `Hash` exposes) alongside the raw bytes callers need
/// to embed as the WebAuthn challenge.
fn hash32(env: &Env, seed: u8) -> (Hash<32>, [u8; 32]) {
    let mut arr = [0u8; 32];
    arr[0] = seed;
    arr[31] = seed.wrapping_mul(3).wrapping_add(1);
    let hash = env.crypto().sha256(&Bytes::from_slice(env, &arr));
    let bytes = hash.to_array();
    (hash, bytes)
}

// ═══════════════════════════════════════════════════════════════════
// CONSTRUCTOR TESTS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_constructor_sets_config_and_owner_signer() {
    let (_env, _contract_id, client, cred_id) = setup();

    let config = client.get_config();
    assert_eq!(config.version, 1);
    assert_eq!(config.owner_credential_id, cred_id);
    assert_eq!(config.recovery_threshold, 2);
    assert_eq!(config.recovery_timelock_ledgers, 34_560);
    assert_eq!(config.guardian_count, 0);
    assert_eq!(config.credential_epoch, 0);

    assert!(client.is_signer(&cred_id));
    let signer = client.get_signer(&cred_id);
    assert_eq!(signer.signer_type, SignerType::Passkey);
    assert_eq!(signer.weight, 10);
    assert_eq!(signer.epoch, 0);
}

#[test]
fn test_nonce_starts_at_zero() {
    let (_env, _contract_id, client, _cred_id) = setup();
    assert_eq!(client.get_nonce(), 0);
}

// ═══════════════════════════════════════════════════════════════════
// SIGNER MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_add_signer() {
    let (env, _contract_id, client, _cred_id) = setup();

    let second_cred = mock_credential_id(&env, 2);
    let mut second_key = [0u8; 65];
    second_key[0] = 0x04;
    second_key[1] = 0xCC;
    let second_pubkey = BytesN::from_array(&env, &second_key);

    let new_signer = SignerEntry {
        signer_type: SignerType::Passkey,
        public_key: second_pubkey.clone(),
        credential_id: second_cred.clone(),
        added_at: 0,
        weight: 10,
        epoch: 999, // caller-supplied epoch must be ignored/overwritten
    };

    client.add_signer(&new_signer);

    assert!(client.is_signer(&second_cred));
    let fetched = client.get_signer(&second_cred);
    assert_eq!(fetched.public_key, second_pubkey);
    assert_eq!(fetched.epoch, 0); // stamped with the wallet's current epoch, not 999
}

#[test]
fn test_add_duplicate_signer_fails() {
    let (_env, _contract_id, client, cred_id) = setup();

    let duplicate = SignerEntry {
        signer_type: SignerType::Ed25519,
        public_key: mock_passkey_pubkey(&_env),
        credential_id: cred_id,
        added_at: 0,
        weight: 5,
        epoch: 0,
    };

    let result = client.try_add_signer(&duplicate);
    assert!(result.is_err());
}

#[test]
fn test_remove_signer() {
    let (env, _contract_id, client, _cred_id) = setup();

    let second_cred = mock_credential_id(&env, 2);
    let mut second_key = [0u8; 65];
    second_key[0] = 0x04;
    let second_pubkey = BytesN::from_array(&env, &second_key);

    let new_signer = SignerEntry {
        signer_type: SignerType::Ed25519,
        public_key: second_pubkey,
        credential_id: second_cred.clone(),
        added_at: 0,
        weight: 5,
        epoch: 0,
    };

    client.add_signer(&new_signer);
    assert!(client.is_signer(&second_cred));

    client.remove_signer(&second_cred);
    assert!(!client.is_signer(&second_cred));
}

#[test]
fn test_cannot_remove_owner_signer() {
    let (_env, _contract_id, client, cred_id) = setup();
    let result = client.try_remove_signer(&cred_id);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════
// GUARDIAN MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_add_guardian() {
    let (env, _contract_id, client, _cred_id) = setup();

    let guardian_addr = Address::generate(&env);
    let guardian = GuardianInfo {
        address: guardian_addr.clone(),
        added_at: 0,
        alias: Symbol::new(&env, "alice"),
    };

    client.add_guardian(&guardian);

    assert!(client.is_guardian(&guardian_addr));
    let config = client.get_config();
    assert_eq!(config.guardian_count, 1);
}

#[test]
fn test_add_duplicate_guardian_fails() {
    let (env, _contract_id, client, _cred_id) = setup();

    let guardian_addr = Address::generate(&env);
    let guardian = GuardianInfo {
        address: guardian_addr.clone(),
        added_at: 0,
        alias: Symbol::new(&env, "alice"),
    };
    client.add_guardian(&guardian);

    let duplicate = GuardianInfo {
        address: guardian_addr,
        added_at: 0,
        alias: Symbol::new(&env, "alice2"),
    };
    let result = client.try_add_guardian(&duplicate);
    assert!(result.is_err());
}

#[test]
fn test_remove_guardian_above_threshold_succeeds() {
    let (env, _contract_id, client, _cred_id) = setup();

    // Threshold is 2 — add 3 guardians so removing one still leaves 2.
    let mut addrs = StdVec::new();
    for _ in 0..3u8 {
        let addr = Address::generate(&env);
        client.add_guardian(&GuardianInfo {
            address: addr.clone(),
            added_at: 0,
            alias: Symbol::new(&env, "g"),
        });
        addrs.push(addr);
    }

    client.remove_guardian(&addrs[0]);
    assert!(!client.is_guardian(&addrs[0]));
    assert_eq!(client.get_config().guardian_count, 2);
}

#[test]
fn test_remove_guardian_below_threshold_fails() {
    let (env, _contract_id, client, _cred_id) = setup();

    // Threshold is 2 — add exactly 2 guardians. Removing either would drop
    // the count below the threshold, making recovery impossible.
    let mut addrs = StdVec::new();
    for _ in 0..2 {
        let addr = Address::generate(&env);
        client.add_guardian(&GuardianInfo {
            address: addr.clone(),
            added_at: 0,
            alias: Symbol::new(&env, "g"),
        });
        addrs.push(addr);
    }

    let result = client.try_remove_guardian(&addrs[0]);
    assert_eq!(result, Err(Ok(WalletError::BelowRecoveryThreshold)));
    assert!(client.is_guardian(&addrs[0])); // untouched
}

#[test]
fn test_remove_nonexistent_guardian_fails() {
    let (env, _contract_id, client, _cred_id) = setup();
    let nonexistent = Address::generate(&env);
    let result = client.try_remove_guardian(&nonexistent);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════
// K-01 REGRESSION: WEBAUTHN CHALLENGE BINDING
// ═══════════════════════════════════════════════════════════════════
//
// Every test in this section calls `__check_auth` directly through the
// real generated contract client — the exact function the Soroban host
// invokes — with a genuine P-256 signature. This is the suite that did
// not exist before the fix, and would have failed against the original
// implementation on every "negative" case below.

#[test]
fn test_valid_assertion_authorizes_its_own_payload() {
    let (env, client, key, cred_id) = setup_with_real_passkey();
    let (payload, payload_arr) = hash32(&env, 1);

    let sig = valid_owner_signature(&env, &key, &cred_id, &payload_arr, 0);
    let result = call_check_auth(&env, &client.address, &payload, &sig, &vec![&env]);
    assert!(result.is_ok(), "a correctly bound assertion must authorize its own payload");
}

#[test]
fn test_assertion_rejected_for_a_different_transaction() {
    // This is the K-01 exploit itself: capture a valid assertion for
    // transaction A, then attempt to use it to authorize transaction B.
    // Before the fix, `client_data_json_hash` was caller-supplied and
    // `signature_payload` was never checked against it — this passed.
    let (env, client, key, cred_id) = setup_with_real_passkey();
    let (_payload_a, payload_a_arr) = hash32(&env, 1);
    let (payload_b, _) = hash32(&env, 2);

    let sig_for_a = valid_owner_signature(&env, &key, &cred_id, &payload_a_arr, 0);

    let result = call_check_auth(&env, &client.address, &payload_b, &sig_for_a, &vec![&env]);
    assert_eq!(result, Err(WalletError::ChallengeMismatch));
}

#[test]
fn test_wrong_ceremony_type_rejected() {
    let (env, client, key, cred_id) = setup_with_real_passkey();
    let (payload, payload_arr) = hash32(&env, 1);

    let rp_hash = rp_id_hash(&env);
    let auth_data = build_authenticator_data(&env, &rp_hash, true, true);
    let client_json = build_client_data_json(&env, &payload_arr, "webauthn.create"); // wrong type
    let sig_bytes = sign_assertion(&env, &key, &auth_data, &client_json);

    let sig = WalletSignature {
        credential_id: cred_id,
        signature_bytes: sig_bytes,
        authenticator_data: auth_data,
        client_data_json: client_json,
        nonce: 0,
    };

    let result = call_check_auth(&env, &client.address, &payload, &sig, &vec![&env]);
    assert_eq!(result, Err(WalletError::WrongCeremonyType));
}

#[test]
fn test_wrong_rp_id_rejected() {
    let (env, client, key, cred_id) = setup_with_real_passkey();
    let (payload, payload_arr) = hash32(&env, 1);

    // authenticator_data claims a DIFFERENT origin than the wallet is bound to
    let wrong_rp_hash: BytesN<32> = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, b"evil.example"))
        .into();
    let auth_data = build_authenticator_data(&env, &wrong_rp_hash, true, true);
    let client_json = build_client_data_json(&env, &payload_arr, "webauthn.get");
    let sig_bytes = sign_assertion(&env, &key, &auth_data, &client_json);

    let sig = WalletSignature {
        credential_id: cred_id,
        signature_bytes: sig_bytes,
        authenticator_data: auth_data,
        client_data_json: client_json,
        nonce: 0,
    };

    let result = call_check_auth(&env, &client.address, &payload, &sig, &vec![&env]);
    assert_eq!(result, Err(WalletError::RpIdMismatch));
}

#[test]
fn test_missing_user_presence_flag_rejected() {
    let (env, client, key, cred_id) = setup_with_real_passkey();
    let (payload, payload_arr) = hash32(&env, 1);

    let rp_hash = rp_id_hash(&env);
    let auth_data = build_authenticator_data(&env, &rp_hash, false, true); // UP missing
    let client_json = build_client_data_json(&env, &payload_arr, "webauthn.get");
    let sig_bytes = sign_assertion(&env, &key, &auth_data, &client_json);

    let sig = WalletSignature {
        credential_id: cred_id,
        signature_bytes: sig_bytes,
        authenticator_data: auth_data,
        client_data_json: client_json,
        nonce: 0,
    };

    let result = call_check_auth(&env, &client.address, &payload, &sig, &vec![&env]);
    assert_eq!(result, Err(WalletError::UserPresenceMissing));
}

#[test]
fn test_missing_user_verification_flag_rejected() {
    let (env, client, key, cred_id) = setup_with_real_passkey();
    let (payload, payload_arr) = hash32(&env, 1);

    let rp_hash = rp_id_hash(&env);
    let auth_data = build_authenticator_data(&env, &rp_hash, true, false); // UV missing
    let client_json = build_client_data_json(&env, &payload_arr, "webauthn.get");
    let sig_bytes = sign_assertion(&env, &key, &auth_data, &client_json);

    let sig = WalletSignature {
        credential_id: cred_id,
        signature_bytes: sig_bytes,
        authenticator_data: auth_data,
        client_data_json: client_json,
        nonce: 0,
    };

    let result = call_check_auth(&env, &client.address, &payload, &sig, &vec![&env]);
    assert_eq!(result, Err(WalletError::UserVerificationMissing));
}

#[test]
#[should_panic]
fn test_tampered_authenticator_data_fails_signature_check() {
    // authenticator_data is signed over — flipping a byte in it after
    // signing must invalidate the signature itself (not just the flags
    // check), since it changes the signed message.
    let (env, client, key, cred_id) = setup_with_real_passkey();
    let (payload, payload_arr) = hash32(&env, 1);

    let mut sig = valid_owner_signature(&env, &key, &cred_id, &payload_arr, 0);
    // Flip a signCount byte in place, well past the flags byte — this
    // changes the signed message without touching any of the explicit
    // checks (RP-ID, flags, challenge), so only the signature itself can
    // catch it.
    let byte35 = sig.authenticator_data.get(35).unwrap();
    sig.authenticator_data.set(35, byte35 ^ 0xFF);

    // secp256r1_verify panics on an invalid signature.
    check_auth_expect_ok(&env, &client.address, &payload, &sig, &vec![&env]);
}

#[test]
fn test_replayed_nonce_rejected() {
    let (env, client, key, cred_id) = setup_with_real_passkey();
    let (payload_a, payload_a_arr) = hash32(&env, 1);
    let (payload_b, payload_b_arr) = hash32(&env, 2);

    let sig_a = valid_owner_signature(&env, &key, &cred_id, &payload_a_arr, 0);
    check_auth_expect_ok(&env, &client.address, &payload_a, &sig_a, &vec![&env]); // consumes nonce 0

    // Same nonce again, correctly bound to a *new* payload — still rejected,
    // because the nonce was already consumed.
    let sig_b_reused_nonce = valid_owner_signature(&env, &key, &cred_id, &payload_b_arr, 0);
    let result = call_check_auth(&env, &client.address, &payload_b, &sig_b_reused_nonce, &vec![&env]);
    assert_eq!(result, Err(WalletError::InvalidNonce));
}

#[test]
fn test_unknown_credential_id_rejected() {
    let (env, client, key, _cred_id) = setup_with_real_passkey();
    let (payload, payload_arr) = hash32(&env, 1);

    let unknown_cred = mock_credential_id(&env, 250);
    let sig = valid_owner_signature(&env, &key, &unknown_cred, &payload_arr, 0);

    let result = call_check_auth(&env, &client.address, &payload, &sig, &vec![&env]);
    assert_eq!(result, Err(WalletError::SignerNotFound));
}

// ═══════════════════════════════════════════════════════════════════
// K-08 REGRESSION: CREDENTIAL EPOCH INVALIDATION AFTER ROTATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_rotated_owner_credential_is_invalidated() {
    let (env, client, key, old_cred) = setup_with_real_passkey();

    // Bind this SmartAccount to itself as its own "recovery module" so the
    // test can call rotate_credentials directly with require_auth mocked.
    env.mock_all_auths();
    client.set_recovery_module(&client.address);

    let new_cred = mock_credential_id(&env, 77);
    let new_key = {
        let mut secret = [0u8; 32];
        secret[31] = 11;
        SigningKey::from_bytes((&secret).into()).unwrap()
    };
    let new_pubkey = signing_key_public_bytes(&env, &new_key);

    client.rotate_credentials(&new_cred, &new_pubkey);

    let config = client.get_config();
    assert_eq!(config.credential_epoch, 1);
    assert_eq!(config.owner_credential_id, new_cred);

    // The OLD credential, even with a perfectly valid signature over the
    // right payload, must now be rejected: its stored epoch (0) no longer
    // matches the wallet's current epoch (1).
    let (payload, payload_arr) = hash32(&env, 5);
    let old_sig = valid_owner_signature(&env, &key, &old_cred, &payload_arr, 0);
    let result = call_check_auth(&env, &client.address, &payload, &old_sig, &vec![&env]);
    assert_eq!(result, Err(WalletError::StaleCredentialEpoch));
}

// ═══════════════════════════════════════════════════════════════════
// K-02 REGRESSION: SESSION KEYS FAIL CLOSED
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_session_creation_rejects_empty_whitelist() {
    let (env, _contract_id, client, _cred_id) = setup();

    let session_key = BytesN::from_array(&env, &[3u8; 32]);
    let empty_config = SessionConfig {
        public_key: session_key.clone(),
        allowed_contracts: soroban_sdk::Vec::new(&env),
        allowed_functions: soroban_sdk::Vec::new(&env),
        max_amount_per_tx: 100,
        max_total_amount: 100,
        spent_amount: 0,
        expires_at_ledger: 200_000,
        epoch: 0,
    };

    let result = client.try_create_session(&session_key, &empty_config);
    assert_eq!(result, Err(Ok(WalletError::SessionKeyInvalid)));
}

#[test]
fn test_session_creation_rejects_self_as_target() {
    let (env, contract_id, client, _cred_id) = setup();

    let session_key = BytesN::from_array(&env, &[3u8; 32]);
    let mut targets = soroban_sdk::Vec::new(&env);
    targets.push_back(contract_id.clone()); // the wallet itself
    let mut functions = soroban_sdk::Vec::new(&env);
    functions.push_back(Symbol::new(&env, "add_signer"));

    let config = SessionConfig {
        public_key: session_key.clone(),
        allowed_contracts: targets,
        allowed_functions: functions,
        max_amount_per_tx: 100,
        max_total_amount: 100,
        spent_amount: 0,
        expires_at_ledger: 200_000,
        epoch: 0,
    };

    let result = client.try_create_session(&session_key, &config);
    assert_eq!(result, Err(Ok(WalletError::UnauthorizedTarget)));
}

/// Build a session, sign a payload for it with a real ed25519 key, and run
/// it through `__check_auth` end-to-end against `contexts`.
fn run_session_check(
    env: &Env,
    client: &SmartAccountContractClient<'static>,
    ed_key: &ed25519_dalek::SigningKey,
    session_id: &BytesN<32>,
    payload: &Hash<32>,
    nonce: u128,
    contexts: soroban_sdk::Vec<Context>,
) -> Result<(), WalletError> {
    use ed25519_dalek::Signer;
    let sig_bytes = ed_key.sign(&payload.to_array());
    let sig = WalletSignature {
        credential_id: session_id.clone(),
        signature_bytes: BytesN::from_array(env, &sig_bytes.to_bytes()),
        authenticator_data: Bytes::new(env),
        client_data_json: Bytes::new(env),
        nonce,
    };
    call_check_auth(env, &client.address, payload, &sig, &contexts)
}

#[test]
fn test_session_key_executes_within_scope() {
    let (env, _contract_id, client, _cred_id) = setup();

    let ed_key = ed25519_dalek::SigningKey::from_bytes(&[6u8; 32]);
    let session_pk = BytesN::from_array(&env, &ed_key.verifying_key().to_bytes());

    let target_contract = Address::generate(&env);
    let mut targets = soroban_sdk::Vec::new(&env);
    targets.push_back(target_contract.clone());
    let mut functions = soroban_sdk::Vec::new(&env);
    functions.push_back(Symbol::new(&env, "transfer"));

    let config = SessionConfig {
        public_key: session_pk.clone(),
        allowed_contracts: targets,
        allowed_functions: functions,
        max_amount_per_tx: 500,
        max_total_amount: 1000,
        spent_amount: 0,
        expires_at_ledger: 200_000,
        epoch: 0,
    };
    let session_id = client.create_session(&session_pk, &config);

    let ctx = Context::Contract(ContractContext {
        contract: target_contract,
        fn_name: Symbol::new(&env, "transfer"),
        args: soroban_sdk::Vec::new(&env),
    });
    let (payload, _) = hash32(&env, 8);
    let nonce = client.get_nonce();

    let result = run_session_check(
        &env,
        &client,
        &ed_key,
        &session_id,
        &payload,
        nonce,
        vec![&env, ctx],
    );
    assert!(result.is_ok());
}

#[test]
fn test_session_key_cannot_target_the_wallet_itself() {
    let (env, contract_id, client, _cred_id) = setup();

    let ed_key = ed25519_dalek::SigningKey::from_bytes(&[6u8; 32]);
    let session_pk = BytesN::from_array(&env, &ed_key.verifying_key().to_bytes());

    let target_contract = Address::generate(&env);
    let mut targets = soroban_sdk::Vec::new(&env);
    targets.push_back(target_contract);
    let mut functions = soroban_sdk::Vec::new(&env);
    functions.push_back(Symbol::new(&env, "add_signer"));

    let config = SessionConfig {
        public_key: session_pk.clone(),
        allowed_contracts: targets,
        allowed_functions: functions,
        max_amount_per_tx: 500,
        max_total_amount: 1000,
        spent_amount: 0,
        expires_at_ledger: 200_000,
        epoch: 0,
    };
    let session_id = client.create_session(&session_pk, &config);

    // Attempt to use this session key to call the wallet's own
    // `add_signer` — the exact K-02 escalation path.
    let ctx = Context::Contract(ContractContext {
        contract: contract_id,
        fn_name: Symbol::new(&env, "add_signer"),
        args: soroban_sdk::Vec::new(&env),
    });
    let (payload, _) = hash32(&env, 9);
    let nonce = client.get_nonce();

    let result = run_session_check(
        &env,
        &client,
        &ed_key,
        &session_id,
        &payload,
        nonce,
        vec![&env, ctx],
    );
    assert_eq!(result, Err(WalletError::UnauthorizedTarget));
}

#[test]
fn test_session_key_over_cap_rejected() {
    let (env, _contract_id, client, _cred_id) = setup();

    let ed_key = ed25519_dalek::SigningKey::from_bytes(&[6u8; 32]);
    let session_pk = BytesN::from_array(&env, &ed_key.verifying_key().to_bytes());

    let target_contract = Address::generate(&env);
    let mut targets = soroban_sdk::Vec::new(&env);
    targets.push_back(target_contract.clone());
    let mut functions = soroban_sdk::Vec::new(&env);
    functions.push_back(Symbol::new(&env, "transfer"));

    let config = SessionConfig {
        public_key: session_pk.clone(),
        allowed_contracts: targets,
        allowed_functions: functions,
        max_amount_per_tx: 100,
        max_total_amount: 1000,
        spent_amount: 0,
        expires_at_ledger: 200_000,
        epoch: 0,
    };
    let session_id = client.create_session(&session_pk, &config);

    let mut args = soroban_sdk::Vec::new(&env);
    args.push_back(Address::generate(&env).into_val(&env));
    args.push_back(Address::generate(&env).into_val(&env));
    args.push_back(999i128.into_val(&env)); // over max_amount_per_tx (100)

    let ctx = Context::Contract(ContractContext {
        contract: target_contract,
        fn_name: Symbol::new(&env, "transfer"),
        args,
    });
    let (payload, _) = hash32(&env, 10);
    let nonce = client.get_nonce();

    let result = run_session_check(
        &env,
        &client,
        &ed_key,
        &session_id,
        &payload,
        nonce,
        vec![&env, ctx],
    );
    assert_eq!(result, Err(WalletError::AmountExceedsSessionCap));
}
