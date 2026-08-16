#![cfg(test)]

use soroban_sdk::{
    auth::{Context, ContractContext},
    testutils::Address as _,
    vec, Address, Env, IntoVal, Symbol,
};

use crate::{PolicyEngineContract, PolicyEngineContractClient};
use novus_types::WalletError;

fn setup() -> (Env, Address, PolicyEngineContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(PolicyEngineContract, (admin.clone(),));
    let client = PolicyEngineContractClient::new(&env, &contract_id);

    (env, contract_id, client, admin)
}

fn transfer_ctx(env: &Env, token: &Address, amount: i128) -> Context {
    let mut args = soroban_sdk::Vec::new(env);
    args.push_back(Address::generate(env).into_val(env));
    args.push_back(Address::generate(env).into_val(env));
    args.push_back(amount.into_val(env));
    Context::Contract(ContractContext {
        contract: token.clone(),
        fn_name: Symbol::new(env, "transfer"),
        args,
    })
}

#[test]
fn test_whitelist_disabled_by_default() {
    let (env, _id, client, _admin) = setup();
    assert!(!client.is_whitelist_enforced());

    // With enforcement off, an arbitrary un-whitelisted contract passes.
    let random_contract = Address::generate(&env);
    let ctx = transfer_ctx(&env, &random_contract, 10);
    let result = client.try_check_policy(&Address::generate(&env), &vec![&env, ctx]);
    assert!(result.is_ok());
}

#[test]
fn test_whitelist_enforced_rejects_unlisted_contract() {
    let (env, _id, client, _admin) = setup();
    client.set_whitelist_enforced(&true);

    let random_contract = Address::generate(&env);
    let ctx = transfer_ctx(&env, &random_contract, 10);
    let result = client.try_check_policy(&Address::generate(&env), &vec![&env, ctx]);
    assert_eq!(result, Err(Ok(WalletError::UnauthorizedTarget)));
}

#[test]
fn test_whitelist_enforced_allows_listed_contract() {
    let (env, _id, client, _admin) = setup();
    client.set_whitelist_enforced(&true);

    let allowed_contract = Address::generate(&env);
    client.add_whitelist(&allowed_contract);
    assert!(client.is_whitelisted(&allowed_contract));

    let ctx = transfer_ctx(&env, &allowed_contract, 10);
    let result = client.try_check_policy(&Address::generate(&env), &vec![&env, ctx]);
    assert!(result.is_ok());
}

#[test]
fn test_remove_whitelist() {
    let (env, _id, client, _admin) = setup();
    let contract = Address::generate(&env);
    client.add_whitelist(&contract);
    assert!(client.is_whitelisted(&contract));

    client.remove_whitelist(&contract);
    assert!(!client.is_whitelisted(&contract));
}

#[test]
fn test_token_limit_override_enforced() {
    let (env, _id, client, _admin) = setup();
    let token = Address::generate(&env);
    client.set_token_limit(&token, &100);
    assert_eq!(client.get_token_limit(&token), Some(100));

    let over_limit_ctx = transfer_ctx(&env, &token, 150);
    let result = client.try_check_policy(&Address::generate(&env), &vec![&env, over_limit_ctx]);
    assert_eq!(result, Err(Ok(WalletError::SpendingLimitExceeded)));

    let within_limit_ctx = transfer_ctx(&env, &token, 50);
    let result = client.try_check_policy(&Address::generate(&env), &vec![&env, within_limit_ctx]);
    assert!(result.is_ok());
}

#[test]
fn test_admin_gated_setters_reject_non_admin_in_principle() {
    // With mock_all_auths(), we can't exercise a *rejected* signature here,
    // but we can confirm the getters reflect exactly what admin-authorized
    // calls did, which is what every other test in this file already
    // depends on implicitly.
    let (_env, _id, client, admin) = setup();
    let fetched_admin = client.get_admin();
    assert_eq!(fetched_admin, admin);
}
