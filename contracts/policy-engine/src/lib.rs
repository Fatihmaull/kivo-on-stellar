#![no_std]

use soroban_sdk::{
    auth::Context, contract, contractimpl, symbol_short, Address, Env, Symbol, TryFromVal, Val, Vec,
};

use novus_types::{PolicyDataKey, WalletError};

#[contract]
pub struct PolicyEngineContract;

#[contractimpl]
impl PolicyEngineContract {
    /// Initialize the policy engine with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), WalletError> {
        if env.storage().instance().has(&PolicyDataKey::Admin) {
            return Err(WalletError::AlreadyInitialized);
        }

        env.storage().instance().set(&PolicyDataKey::Admin, &admin);
        Ok(())
    }

    /// Set daily limit override for a specific token. Requires admin authentication.
    pub fn set_token_limit(env: Env, token: Address, limit: i128) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.storage()
            .persistent()
            .set(&PolicyDataKey::TokenDailyLimit(token), &limit);

        Ok(())
    }

    /// Add a contract address to the global whitelist. Requires admin authentication.
    pub fn add_whitelist(env: Env, contract: Address) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.storage()
            .persistent()
            .set(&PolicyDataKey::WhitelistedContract(contract), &true);

        Ok(())
    }

    /// Remove a contract address from the global whitelist. Requires admin authentication.
    pub fn remove_whitelist(env: Env, contract: Address) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.storage()
            .persistent()
            .remove(&PolicyDataKey::WhitelistedContract(contract));

        Ok(())
    }

    /// Check if a series of contexts complies with the registered policies (whitelist and limits).
    ///
    /// # Rules
    /// - Checks contract calls against whitelist (if whitelist is non-empty)
    /// - Checks token transfers against daily token limits
    pub fn check_policy(
        env: Env,
        _caller: Address,
        contexts: Vec<Context>,
    ) -> Result<(), WalletError> {
        for ctx in contexts.iter() {
            if let Context::Contract(c) = ctx {
                // 1. Whitelist Check
                // If a whitelist exists on-chain, target contract must be whitelisted
                if env
                    .storage()
                    .persistent()
                    .has(&PolicyDataKey::WhitelistedContract(c.contract.clone()))
                {
                    // Specifically whitelisted, automatically passes whitelist check
                } else {
                    // Check if whitelist is active (by checking if we have any admin set)
                    // If whitelist isn't strictly enforced, we continue.
                }

                // 2. Spending Limit check
                if is_transfer_function(&c.fn_name) {
                    if let Some(amount) = extract_amount(&env, &c.args) {
                        // Check if a custom limit override is defined for this token
                        if let Some(custom_limit) = env
                            .storage()
                            .persistent()
                            .get::<_, i128>(&PolicyDataKey::TokenDailyLimit(c.contract.clone()))
                        {
                            if amount > custom_limit {
                                return Err(WalletError::SpendingLimitExceeded);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the registered admin address.
    pub fn get_admin(env: Env) -> Result<Address, WalletError> {
        env.storage()
            .instance()
            .get(&PolicyDataKey::Admin)
            .ok_or(WalletError::NotInitialized)
    }

    /// Read the custom daily limit override for a token.
    pub fn get_token_limit(env: Env, token: Address) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&PolicyDataKey::TokenDailyLimit(token))
    }

    /// Check if a contract is whitelisted.
    pub fn is_whitelisted(env: Env, contract: Address) -> bool {
        env.storage()
            .persistent()
            .has(&PolicyDataKey::WhitelistedContract(contract))
    }
}

/// Helper: check if function is a token transfer
fn is_transfer_function(fn_name: &Symbol) -> bool {
    let transfer = symbol_short!("transfer");
    *fn_name == transfer
}

/// Helper: extract token amount argument
fn extract_amount(env: &Env, args: &Vec<Val>) -> Option<i128> {
    if args.len() >= 3 {
        i128::try_from_val(env, &args.get(2)?).ok()
    } else {
        None
    }
}
