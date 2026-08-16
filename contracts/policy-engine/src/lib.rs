#![no_std]

use soroban_sdk::{
    auth::Context, contract, contractimpl, symbol_short, Address, Env, Symbol, TryFromVal, Val,
    Vec,
};

use novus_types::{PolicyDataKey, WalletError};

/// ═══════════════════════════════════════════════════════════════════════
/// PolicyEngine Contract
/// ═══════════════════════════════════════════════════════════════════════
///
/// Optional second policy layer a SmartAccount can register via
/// `set_policy_engine`. Enforces a global contract whitelist (only active
/// once explicitly turned on, so adopting this contract can't lock a
/// wallet out before it's been curated) and per-token daily limit
/// overrides on top of the wallet's own built-in limit.
#[contract]
pub struct PolicyEngineContract;

#[contractimpl]
impl PolicyEngineContract {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&PolicyDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&PolicyDataKey::WhitelistEnforced, &false);
    }

    /// Turn whitelist enforcement on/off. Requires admin authentication.
    pub fn set_whitelist_enforced(env: Env, enforced: bool) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&PolicyDataKey::WhitelistEnforced, &enforced);
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

    /// Check a series of contexts against the registered policies.
    ///
    /// # Rules
    /// - If the whitelist is enforced, every contract target in `contexts`
    ///   must be whitelisted, or this call fails.
    /// - Independent of the whitelist, any token transfer with a per-token
    ///   limit override must stay within it.
    pub fn check_policy(
        env: Env,
        _caller: Address,
        contexts: Vec<Context>,
    ) -> Result<(), WalletError> {
        let whitelist_enforced: bool = env
            .storage()
            .instance()
            .get(&PolicyDataKey::WhitelistEnforced)
            .unwrap_or(false);

        for ctx in contexts.iter() {
            if let Context::Contract(c) = ctx {
                if whitelist_enforced
                    && !env
                        .storage()
                        .persistent()
                        .has(&PolicyDataKey::WhitelistedContract(c.contract.clone()))
                {
                    return Err(WalletError::UnauthorizedTarget);
                }

                if is_transfer_function(&c.fn_name) {
                    if let Some(amount) = extract_amount(&env, &c.args) {
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

    pub fn get_admin(env: Env) -> Result<Address, WalletError> {
        env.storage()
            .instance()
            .get(&PolicyDataKey::Admin)
            .ok_or(WalletError::NotInitialized)
    }

    pub fn is_whitelist_enforced(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&PolicyDataKey::WhitelistEnforced)
            .unwrap_or(false)
    }

    pub fn get_token_limit(env: Env, token: Address) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&PolicyDataKey::TokenDailyLimit(token))
    }

    pub fn is_whitelisted(env: Env, contract: Address) -> bool {
        env.storage()
            .persistent()
            .has(&PolicyDataKey::WhitelistedContract(contract))
    }
}

fn is_transfer_function(fn_name: &Symbol) -> bool {
    let transfer = symbol_short!("transfer");
    *fn_name == transfer
}

fn extract_amount(env: &Env, args: &Vec<Val>) -> Option<i128> {
    if args.len() >= 3 {
        i128::try_from_val(env, &args.get(2)?).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod test;
