#![cfg(test)]
extern crate std;

/// Full-stack recovery flow: a real `smart-account` deployment (not a
/// stub) bound to a real `recovery-module`, exercising the actual
/// cross-contract `is_guardian` / `get_config` / `rotate_credentials`
/// calls both contracts make against each other. This is what a K-04
/// regression test looks like: the timelock now lives on the persistent
/// `RecoveryProposal`, so this suite can advance the ledger arbitrarily
/// far past a proposal's creation without the deadline ever going missing.
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Bytes, BytesN, Env,
};

use novus_smart_account::{SmartAccountContract, SmartAccountContractClient};

use crate::{RecoveryModuleContract, RecoveryModuleContractClient};
use novus_types::{GuardianInfo, ProposalStatus, WalletError};

const RP_ID: &str = "kivo.app";

struct Fixture {
    env: Env,
    wallet: SmartAccountContractClient<'static>,
    recovery: RecoveryModuleContractClient<'static>,
    guardians: std::vec::Vec<Address>,
}

fn set_ledger(env: &Env, sequence: u32) {
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000,
        protocol_version: 22,
        sequence_number: sequence,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1_000,
        max_entry_ttl: 10_000_000_00,
    });
}

/// Deploy a real SmartAccount (threshold 2-of-3) plus its RecoveryModule,
/// wire them together, and register 3 guardians.
fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 100_000);

    let mut owner_cred = [0u8; 32];
    owner_cred[0] = 1;
    let mut owner_pk = [0u8; 65];
    owner_pk[0] = 0x04;
    let rp_id_hash: BytesN<32> = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, RP_ID.as_bytes()))
        .into();

    let wallet_id = env.register(
        SmartAccountContract,
        (
            BytesN::from_array(&env, &owner_cred),
            BytesN::from_array(&env, &owner_pk),
            rp_id_hash,
            2u32,     // recovery_threshold: 2-of-3
            1_000u32, // recovery_timelock_ledgers
            1_000_000_0000000i128,
        ),
    );
    let wallet = SmartAccountContractClient::new(&env, &wallet_id);

    let recovery_id = env.register(RecoveryModuleContract, (wallet_id.clone(),));
    let recovery = RecoveryModuleContractClient::new(&env, &recovery_id);

    wallet.set_recovery_module(&recovery_id);

    let mut guardians = std::vec::Vec::new();
    for _ in 0..3u8 {
        let addr = Address::generate(&env);
        wallet.add_guardian(&GuardianInfo {
            address: addr.clone(),
            added_at: 0,
            alias: soroban_sdk::Symbol::new(&env, "g"),
        });
        guardians.push(addr);
    }

    Fixture {
        env,
        wallet,
        recovery,
        guardians,
    }
}

fn new_credential(env: &Env, seed: u8) -> (BytesN<32>, BytesN<65>) {
    let mut cred = [0u8; 32];
    cred[0] = seed;
    let mut pk = [0u8; 65];
    pk[0] = 0x04;
    pk[1] = seed;
    (BytesN::from_array(env, &cred), BytesN::from_array(env, &pk))
}

#[test]
fn test_propose_requires_registered_guardian() {
    let f = setup();
    let (new_cred, new_pk) = new_credential(&f.env, 9);
    let stranger = Address::generate(&f.env);

    let result = f
        .recovery
        .try_propose_recovery(&stranger, &new_cred, &new_pk);
    assert_eq!(result, Err(Ok(WalletError::Unauthorized)));
}

#[test]
fn test_full_recovery_lifecycle() {
    let f = setup();
    let (new_cred, new_pk) = new_credential(&f.env, 9);

    // 1 — first guardian proposes.
    let proposal_id =
        f.recovery
            .propose_recovery(&f.guardians[0], &new_cred, &new_pk);
    let proposal = f.recovery.get_proposal(&proposal_id);
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(proposal.timelock_expires_at, 0);

    // 2 — second guardian approves, crossing the 2-of-3 threshold.
    let status = f.recovery.approve_recovery(&f.guardians[1], &proposal_id);
    assert_eq!(status, ProposalStatus::TimelockStarted);

    let proposal = f.recovery.get_proposal(&proposal_id);
    assert_eq!(proposal.timelock_expires_at, 100_000 + 1_000);

    // 3 — executing before the timelock expires must fail.
    let result = f.recovery.try_execute_recovery(&proposal_id);
    assert_eq!(result, Err(Ok(WalletError::TimelockActive)));

    // 4 — advance the ledger well past the timelock. Because the deadline
    // lives on the Persistent proposal (not a Temporary entry), this can
    // be an arbitrarily large jump without the deadline ever having been
    // silently archived out from under the proposal.
    set_ledger(&f.env, 100_000 + 1_000 + 50_000);

    f.recovery.execute_recovery(&proposal_id);

    let proposal = f.recovery.get_proposal(&proposal_id);
    assert_eq!(proposal.status, ProposalStatus::Executed);

    // 5 — the wallet's owner credential and epoch actually rotated.
    let config = f.wallet.get_config();
    assert_eq!(config.owner_credential_id, new_cred);
    assert_eq!(config.credential_epoch, 1);
    assert!(f.wallet.is_signer(&new_cred));
}

#[test]
fn test_duplicate_approval_from_same_guardian_does_not_double_count() {
    let f = setup();
    let (new_cred, new_pk) = new_credential(&f.env, 9);

    let proposal_id =
        f.recovery
            .propose_recovery(&f.guardians[0], &new_cred, &new_pk);

    // The same guardian "approving" again (already implicitly approved via
    // the proposal) must not push the proposal past threshold on its own.
    let status = f.recovery.approve_recovery(&f.guardians[0], &proposal_id);
    assert_eq!(status, ProposalStatus::Pending);

    let proposal = f.recovery.get_proposal(&proposal_id);
    assert_eq!(proposal.approvals.len(), 1);
}

#[test]
fn test_cancel_recovery_by_wallet_itself() {
    let f = setup();
    let (new_cred, new_pk) = new_credential(&f.env, 9);

    let proposal_id =
        f.recovery
            .propose_recovery(&f.guardians[0], &new_cred, &new_pk);

    f.recovery.cancel_recovery(&f.wallet.address, &proposal_id);

    let proposal = f.recovery.get_proposal(&proposal_id);
    assert_eq!(proposal.status, ProposalStatus::Cancelled);

    // A cancelled proposal cannot later be approved back to life.
    let result = f.recovery.try_approve_recovery(&f.guardians[1], &proposal_id);
    assert_eq!(result, Err(Ok(WalletError::ProposalAlreadyExecuted)));
}

#[test]
fn test_execute_before_threshold_met_fails() {
    let f = setup();
    let (new_cred, new_pk) = new_credential(&f.env, 9);

    let proposal_id =
        f.recovery
            .propose_recovery(&f.guardians[0], &new_cred, &new_pk);

    // Only 1 of the required 2 approvals is in — still Pending.
    let result = f.recovery.try_execute_recovery(&proposal_id);
    assert_eq!(result, Err(Ok(WalletError::RecoveryNotReady)));
}
