#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, IntoVal, Vec};

use novus_types::{PaymasterDataKey, WalletError};

/// ═══════════════════════════════════════════════════════════════════════
/// Paymaster Contract
/// ═══════════════════════════════════════════════════════════════════════
///
/// Owns the accepted-fee-token registry and margin configuration, and acts
/// as the treasury address fee payments land in. It does **not** pull
/// funds from a wallet itself — that would let a relayer name its own
/// price with nothing capping it. Instead `SmartAccountContract::execute_sponsored`
/// reads `is_accepted_token` from here, transfers the fee itself under the
/// same signature that authorizes the sponsored action, and enforces the
/// user-signed `max_fee` bound before doing so. This contract's job is
/// configuration and custody, not pulling money.
#[contract]
pub struct PaymasterContract;

#[contractimpl]
impl PaymasterContract {
    pub fn __constructor(env: Env, admin: Address, relayer: Address, xlm_token: Address) {
        env.storage().instance().set(&PaymasterDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&PaymasterDataKey::Relayer, &relayer);
        env.storage()
            .instance()
            .set(&PaymasterDataKey::XlmToken, &xlm_token);
        env.storage()
            .instance()
            .set(&PaymasterDataKey::FeeMarginBps, &500u32); // Default 5% fee margin
    }

    /// Set the fee margin in basis points (e.g. 500 = 5% extra). Requires admin authorization.
    pub fn set_fee_margin(env: Env, margin_bps: u32) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&PaymasterDataKey::FeeMarginBps, &margin_bps);
        Ok(())
    }

    /// Replace the relayer address. Requires admin authorization.
    pub fn set_relayer(env: Env, relayer: Address) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&PaymasterDataKey::Relayer, &relayer);
        Ok(())
    }

    /// Add an accepted fee token (like USDC). Requires admin authorization.
    pub fn add_accepted_token(env: Env, token: Address) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        let mut tokens = Self::get_accepted_tokens(env.clone());
        if !tokens.contains(&token) {
            tokens.push_back(token);
            env.storage()
                .instance()
                .set(&PaymasterDataKey::AcceptedTokens, &tokens);
        }

        Ok(())
    }

    /// Remove an accepted fee token. Requires admin authorization.
    pub fn remove_accepted_token(env: Env, token: Address) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        let tokens = Self::get_accepted_tokens(env.clone());
        let mut new_tokens = Vec::new(&env);
        for t in tokens.iter() {
            if t != token {
                new_tokens.push_back(t);
            }
        }

        env.storage()
            .instance()
            .set(&PaymasterDataKey::AcceptedTokens, &new_tokens);
        Ok(())
    }

    /// Sweep collected fee tokens out of the treasury (e.g. rebalance to
    /// XLM off-chain, or forward to an operating account). Requires admin
    /// authorization.
    pub fn reclaim_fees(env: Env, token: Address, to: Address, amount: i128) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.invoke_contract::<()>(
            &token,
            &symbol_short!("transfer"),
            (env.current_contract_address(), to, amount).into_val(&env),
        );

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // GETTERS
    // ═══════════════════════════════════════════════════════════════════

    pub fn get_admin(env: Env) -> Result<Address, WalletError> {
        env.storage()
            .instance()
            .get(&PaymasterDataKey::Admin)
            .ok_or(WalletError::NotInitialized)
    }

    pub fn get_relayer(env: Env) -> Result<Address, WalletError> {
        env.storage()
            .instance()
            .get(&PaymasterDataKey::Relayer)
            .ok_or(WalletError::NotInitialized)
    }

    pub fn get_xlm_token(env: Env) -> Result<Address, WalletError> {
        env.storage()
            .instance()
            .get(&PaymasterDataKey::XlmToken)
            .ok_or(WalletError::NotInitialized)
    }

    pub fn get_fee_margin(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&PaymasterDataKey::FeeMarginBps)
            .unwrap_or(0)
    }

    pub fn get_accepted_tokens(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&PaymasterDataKey::AcceptedTokens)
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn is_accepted_token(env: Env, token: Address) -> bool {
        Self::get_accepted_tokens(env).contains(&token)
    }
}

#[cfg(test)]
mod test;
