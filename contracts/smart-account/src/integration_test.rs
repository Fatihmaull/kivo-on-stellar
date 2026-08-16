#![cfg(test)]

/// Cross-contract integration tests: a real SmartAccount wired to a real
/// Paymaster and a real Stellar Asset Contract test token, exercising
/// `execute_sponsored` end to end — the K-05 fix (a caller-signed `max_fee`
/// ceiling the contract itself enforces, rather than trusting the relayer's
/// quote) only means something if the atomic action-plus-fee transfer
/// actually moves real token balances the way it claims to.
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Bytes, BytesN, Env, IntoVal, Symbol,
};

use crate::{SmartAccountContract, SmartAccountContractClient};
use novus_paymaster::{PaymasterContract, PaymasterContractClient};
use novus_types::WalletError;

const RP_ID: &str = "kivo.app";

struct Fixture {
    env: Env,
    wallet: SmartAccountContractClient<'static>,
    paymaster: PaymasterContractClient<'static>,
    token: TokenClient<'static>,
    relayer: Address,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
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
            2u32,
            34_560u32,
            1_000_000_0000000i128,
        ),
    );
    let wallet = SmartAccountContractClient::new(&env, &wallet_id);

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = sac.address();
    let token = TokenClient::new(&env, &token_address);
    let token_admin_client = StellarAssetClient::new(&env, &token_address);

    let paymaster_id = env.register(
        PaymasterContract,
        (admin.clone(), relayer.clone(), token_address.clone()),
    );
    let paymaster = PaymasterContractClient::new(&env, &paymaster_id);
    paymaster.add_accepted_token(&token_address);

    wallet.set_paymaster(&paymaster_id);

    // Fund the wallet with test tokens, as if the user had already received
    // some USDC-equivalent balance.
    token_admin_client.mint(&wallet_id, &10_000);

    Fixture {
        env,
        wallet,
        paymaster,
        token,
        relayer,
    }
}

#[test]
fn test_execute_sponsored_moves_action_and_fee_atomically() {
    let f = setup();
    let recipient = Address::generate(&f.env);

    let mut transfer_args = soroban_sdk::Vec::new(&f.env);
    transfer_args.push_back(f.wallet.address.clone().into_val(&f.env));
    transfer_args.push_back(recipient.clone().into_val(&f.env));
    transfer_args.push_back(1_000i128.into_val(&f.env));

    f.wallet.execute_sponsored(
        &f.token.address,
        &Symbol::new(&f.env, "transfer"),
        &transfer_args,
        &f.token.address,
        &50i128, // max_fee
        &30i128, // quoted
    );

    assert_eq!(f.token.balance(&recipient), 1_000);
    assert_eq!(f.token.balance(&f.paymaster.address), 30);
    assert_eq!(f.token.balance(&f.wallet.address), 10_000 - 1_000 - 30);
    let _ = f.relayer;
}

#[test]
fn test_execute_sponsored_rejects_fee_over_max() {
    let f = setup();
    let recipient = Address::generate(&f.env);

    let mut transfer_args = soroban_sdk::Vec::new(&f.env);
    transfer_args.push_back(f.wallet.address.clone().into_val(&f.env));
    transfer_args.push_back(recipient.into_val(&f.env));
    transfer_args.push_back(1_000i128.into_val(&f.env));

    let result = f.wallet.try_execute_sponsored(
        &f.token.address,
        &Symbol::new(&f.env, "transfer"),
        &transfer_args,
        &f.token.address,
        &10i128, // max_fee
        &30i128, // quoted — over the user-signed cap
    );

    assert!(matches!(result, Err(Ok(WalletError::FeeExceedsMax))));
    // Nothing moved — the whole call reverted, not just the fee half.
    assert_eq!(f.token.balance(&f.wallet.address), 10_000);
}

#[test]
fn test_execute_sponsored_rejects_unaccepted_fee_token() {
    let f = setup();
    let other_token_admin = Address::generate(&f.env);
    let other_sac = f
        .env
        .register_stellar_asset_contract_v2(other_token_admin);
    let other_token = other_sac.address();

    let recipient = Address::generate(&f.env);
    let mut transfer_args = soroban_sdk::Vec::new(&f.env);
    transfer_args.push_back(f.wallet.address.clone().into_val(&f.env));
    transfer_args.push_back(recipient.into_val(&f.env));
    transfer_args.push_back(100i128.into_val(&f.env));

    let result = f.wallet.try_execute_sponsored(
        &f.token.address,
        &Symbol::new(&f.env, "transfer"),
        &transfer_args,
        &other_token, // never registered with the paymaster
        &50i128,
        &10i128,
    );

    assert!(matches!(result, Err(Ok(WalletError::UnsupportedFeeToken))));
}

#[test]
#[should_panic(expected = "Error(Contract, #302)")]
fn test_policy_engine_whitelist_blocks_unlisted_target() {
    let f = setup();

    let admin = Address::generate(&f.env);
    let policy_id = f
        .env
        .register(novus_policy_engine::PolicyEngineContract, (admin.clone(),));
    let policy = novus_policy_engine::PolicyEngineContractClient::new(&f.env, &policy_id);
    policy.set_whitelist_enforced(&true);
    // Deliberately do NOT whitelist the token contract.

    f.wallet.set_policy_engine(&policy_id);

    // A direct check_auth-driven transfer context should now be blocked by
    // the registered PolicyEngine even though the built-in daily limit
    // alone would have allowed it. `enforce_policies` calls the
    // PolicyEngine via a raw `invoke_contract` (matching the rest of this
    // codebase's cross-contract style), so a rejection there traps the
    // whole call rather than bubbling up as an `Err` — 302 is
    // `WalletError::UnauthorizedTarget`, and a trap is exactly what should
    // happen to a real transaction hitting this policy in production.
    let recipient = Address::generate(&f.env);
    let mut transfer_args = soroban_sdk::Vec::new(&f.env);
    transfer_args.push_back(f.wallet.address.clone().into_val(&f.env));
    transfer_args.push_back(recipient.into_val(&f.env));
    transfer_args.push_back(10i128.into_val(&f.env));

    let ctx = soroban_sdk::auth::Context::Contract(soroban_sdk::auth::ContractContext {
        contract: f.token.address.clone(),
        fn_name: Symbol::new(&f.env, "transfer"),
        args: transfer_args,
    });

    f.env.as_contract(&f.wallet.address, || {
        crate::policy::enforce_policies(&f.env, &vec![&f.env, ctx])
    })
    .ok();
}
