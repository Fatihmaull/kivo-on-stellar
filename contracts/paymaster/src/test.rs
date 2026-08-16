#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

use crate::{PaymasterContract, PaymasterContractClient};

fn setup() -> (Env, PaymasterContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let xlm_token = Address::generate(&env);

    let contract_id = env.register(
        PaymasterContract,
        (admin.clone(), relayer.clone(), xlm_token.clone()),
    );
    let client = PaymasterContractClient::new(&env, &contract_id);

    (env, client, admin, relayer, xlm_token)
}

#[test]
fn test_constructor_sets_default_fee_margin() {
    let (_env, client, admin, relayer, xlm_token) = setup();
    assert_eq!(client.get_fee_margin(), 500);
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_relayer(), relayer);
    assert_eq!(client.get_xlm_token(), xlm_token);
}

#[test]
fn test_set_fee_margin() {
    let (_env, client, ..) = setup();
    client.set_fee_margin(&750);
    assert_eq!(client.get_fee_margin(), 750);
}

#[test]
fn test_accepted_token_lifecycle() {
    let (env, client, ..) = setup();
    let usdc = Address::generate(&env);

    assert!(!client.is_accepted_token(&usdc));
    client.add_accepted_token(&usdc);
    assert!(client.is_accepted_token(&usdc));
    assert_eq!(client.get_accepted_tokens().len(), 1);

    // Adding twice must not duplicate.
    client.add_accepted_token(&usdc);
    assert_eq!(client.get_accepted_tokens().len(), 1);

    client.remove_accepted_token(&usdc);
    assert!(!client.is_accepted_token(&usdc));
}

#[test]
fn test_set_relayer() {
    let (env, client, ..) = setup();
    let new_relayer = Address::generate(&env);
    client.set_relayer(&new_relayer);
    assert_eq!(client.get_relayer(), new_relayer);
}

/// End-to-end with a real (test) Stellar Asset Contract: fund the
/// paymaster treasury, then reclaim funds out to an operator address.
#[test]
fn test_reclaim_fees_moves_real_token_balance() {
    let (env, client, admin, ..) = setup();

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = sac.address();
    let token = TokenClient::new(&env, &token_address);
    let token_admin_client = StellarAssetClient::new(&env, &token_address);

    // Simulate collected fees sitting in the paymaster's treasury.
    token_admin_client.mint(&client.address, &1_000);
    assert_eq!(token.balance(&client.address), 1_000);

    let operator = Address::generate(&env);
    client.reclaim_fees(&token_address, &operator, &400);

    assert_eq!(token.balance(&client.address), 600);
    assert_eq!(token.balance(&operator), 400);
    let _ = admin;
}
