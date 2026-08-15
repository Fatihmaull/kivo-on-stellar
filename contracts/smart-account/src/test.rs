#![cfg(test)]

/// SmartAccount contract test suite.
///
/// Tests cover:
/// - Contract initialization
/// - Signer management (add/remove)
/// - Guardian management (add/remove)
/// - Configuration reads
/// - Nonce management

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env, Symbol,
};

use crate::SmartAccountContract;
use crate::SmartAccountContractClient;
use novus_types::{GuardianInfo, SignerEntry, SignerType};

/// Helper: create a test environment and deploy the SmartAccount contract.
fn setup() -> (Env, Address, SmartAccountContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    // Set up ledger info for consistent testing
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

    let contract_id = env.register(SmartAccountContract, ());
    let client = SmartAccountContractClient::new(&env, &contract_id);

    (env, contract_id, client)
}

/// Helper: generate a mock 65-byte secp256r1 public key.
fn mock_passkey_pubkey(env: &Env) -> BytesN<65> {
    let mut key = [0u8; 65];
    key[0] = 0x04; // Uncompressed point prefix
    key[1] = 0xAA;
    key[2] = 0xBB;
    BytesN::from_array(env, &key)
}

/// Helper: generate a mock 32-byte credential ID.
fn mock_credential_id(env: &Env, seed: u8) -> BytesN<32> {
    let mut id = [0u8; 32];
    id[0] = seed;
    id[1] = seed.wrapping_mul(7);
    BytesN::from_array(env, &id)
}

// ═══════════════════════════════════════════════════════════════════
// INITIALIZATION TESTS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_initialize_success() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    let result = client.try_initialize(
        &cred_id,
        &pubkey,
        &2,      // recovery_threshold
        &34_560, // recovery_timelock_ledgers (~48h)
        &1_000_000_0000000, // default_daily_limit (1M stroops)
    );

    assert!(result.is_ok());
}

#[test]
fn test_initialize_returns_config() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(
        &cred_id,
        &pubkey,
        &2,
        &34_560,
        &1_000_000_0000000,
    );

    let config = client.get_config();
    assert_eq!(config.version, 1);
    assert_eq!(config.owner_credential_id, cred_id);
    assert_eq!(config.recovery_threshold, 2);
    assert_eq!(config.recovery_timelock_ledgers, 34_560);
    assert_eq!(config.guardian_count, 0);
    assert_eq!(config.default_daily_limit, 1_000_000_0000000);
}

#[test]
fn test_initialize_prevents_reinit() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    // Second init should fail
    let result = client.try_initialize(
        &cred_id,
        &pubkey,
        &2,
        &34_560,
        &1_000_000_0000000,
    );

    assert!(result.is_err());
}

#[test]
fn test_initialize_registers_owner_signer() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    // Owner should be registered as a signer
    assert!(client.is_signer(&cred_id));

    let signer = client.get_signer(&cred_id);
    assert_eq!(signer.signer_type, SignerType::Passkey);
    assert_eq!(signer.public_key, pubkey);
    assert_eq!(signer.weight, 10);
}

// ═══════════════════════════════════════════════════════════════════
// NONCE TESTS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_nonce_starts_at_zero() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    assert_eq!(client.get_nonce(), 0);
}

// ═══════════════════════════════════════════════════════════════════
// SIGNER MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_add_signer() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    // Add a second passkey signer
    let second_cred = mock_credential_id(&env, 2);
    let mut second_key = [0u8; 65];
    second_key[0] = 0x04;
    second_key[1] = 0xCC;
    let second_pubkey = BytesN::from_array(&env, &second_key);

    let new_signer = SignerEntry {
        signer_type: SignerType::Passkey,
        public_key: second_pubkey.clone(),
        credential_id: second_cred.clone(),
        added_at: 0, // Will be set by contract
        weight: 10,
    };

    client.add_signer(&new_signer);

    assert!(client.is_signer(&second_cred));
    let fetched = client.get_signer(&second_cred);
    assert_eq!(fetched.public_key, second_pubkey);
}

#[test]
fn test_add_duplicate_signer_fails() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    // Try to add a signer with the same credential_id as the owner
    let duplicate = SignerEntry {
        signer_type: SignerType::Ed25519,
        public_key: pubkey,
        credential_id: cred_id,
        added_at: 0,
        weight: 5,
    };

    let result = client.try_add_signer(&duplicate);
    assert!(result.is_err());
}

#[test]
fn test_remove_signer() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    // Add a second signer
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
    };

    client.add_signer(&new_signer);
    assert!(client.is_signer(&second_cred));

    // Remove the second signer
    client.remove_signer(&second_cred);
    assert!(!client.is_signer(&second_cred));
}

#[test]
fn test_cannot_remove_owner_signer() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    // Attempt to remove the owner — should fail
    let result = client.try_remove_signer(&cred_id);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════
// GUARDIAN MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_add_guardian() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    let guardian_addr = Address::generate(&env);
    let guardian = GuardianInfo {
        address: guardian_addr.clone(),
        added_at: 0,
        alias: Symbol::new(&env, "alice"),
    };

    client.add_guardian(&guardian);

    assert!(client.is_guardian(&guardian_addr));

    // Guardian count should be updated
    let config = client.get_config();
    assert_eq!(config.guardian_count, 1);
}

#[test]
fn test_add_duplicate_guardian_fails() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    let guardian_addr = Address::generate(&env);
    let guardian = GuardianInfo {
        address: guardian_addr.clone(),
        added_at: 0,
        alias: Symbol::new(&env, "alice"),
    };

    client.add_guardian(&guardian);

    // Adding the same guardian again should fail
    let duplicate = GuardianInfo {
        address: guardian_addr,
        added_at: 0,
        alias: Symbol::new(&env, "alice2"),
    };

    let result = client.try_add_guardian(&duplicate);
    assert!(result.is_err());
}

#[test]
fn test_remove_guardian() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    let guardian_addr = Address::generate(&env);
    let guardian = GuardianInfo {
        address: guardian_addr.clone(),
        added_at: 0,
        alias: Symbol::new(&env, "bob"),
    };

    client.add_guardian(&guardian);
    assert!(client.is_guardian(&guardian_addr));

    client.remove_guardian(&guardian_addr);
    assert!(!client.is_guardian(&guardian_addr));

    let config = client.get_config();
    assert_eq!(config.guardian_count, 0);
}

#[test]
fn test_remove_nonexistent_guardian_fails() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &2, &34_560, &1_000_000_0000000);

    let nonexistent = Address::generate(&env);
    let result = client.try_remove_guardian(&nonexistent);
    assert!(result.is_err());
}

#[test]
fn test_add_multiple_guardians() {
    let (env, _contract_id, client) = setup();

    let cred_id = mock_credential_id(&env, 1);
    let pubkey = mock_passkey_pubkey(&env);

    client.initialize(&cred_id, &pubkey, &3, &34_560, &1_000_000_0000000);

    // Add 5 guardians for a 3-of-5 setup
    for i in 0..5u8 {
        let addr = Address::generate(&env);
        let name = match i {
            0 => "alice",
            1 => "bob",
            2 => "carol",
            3 => "dave",
            _ => "eve",
        };
        let guardian = GuardianInfo {
            address: addr,
            added_at: 0,
            alias: Symbol::new(&env, name),
        };
        client.add_guardian(&guardian);
    }

    let config = client.get_config();
    assert_eq!(config.guardian_count, 5);
    assert_eq!(config.recovery_threshold, 3);
}
