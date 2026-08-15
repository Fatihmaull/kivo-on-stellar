#![no_std]

use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, Env, IntoVal, Symbol, Vec,
};

use novus_types::{PaymasterDataKey, WalletError};

#[contract]
pub struct PaymasterContract;

#[contractimpl]
impl PaymasterContract {
    /// Initialize the paymaster with admin, relayer, and native XLM token references.
    pub fn initialize(
        env: Env,
        admin: Address,
        relayer: Address,
        xlm_token: Address,
    ) -> Result<(), WalletError> {
        if env.storage().instance().has(&PaymasterDataKey::Admin) {
            return Err(WalletError::AlreadyInitialized);
        }

        env.storage().instance().set(&PaymasterDataKey::Admin, &admin);
        env.storage().instance().set(&PaymasterDataKey::Relayer, &relayer);
        env.storage().instance().set(&PaymasterDataKey::XlmToken, &xlm_token);
        env.storage().instance().set(&PaymasterDataKey::FeeMarginBps, &500u32); // Default 5% fee margin

        Ok(())
    }

    /// Set the fee margin in basis points (e.g. 500 = 5% extra). Requires admin authorization.
    pub fn set_fee_margin(env: Env, margin_bps: u32) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.storage().instance().set(&PaymasterDataKey::FeeMarginBps, &margin_bps);
        Ok(())
    }

    /// Add an accepted fee token (like USDC). Requires admin authorization.
    pub fn add_accepted_token(env: Env, token: Address) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        let mut tokens = Self::get_accepted_tokens(env.clone());
        if !tokens.contains(&token) {
            tokens.push_back(token);
            env.storage().instance().set(&PaymasterDataKey::AcceptedTokens, &tokens);
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

        env.storage().instance().set(&PaymasterDataKey::AcceptedTokens, &new_tokens);
        Ok(())
    }

    /// Collect fee from the SmartAccount in alternative tokens (e.g. USDC).
    /// Called by the Relayer/Paymaster service during auth wrap.
    pub fn collect_fee(
        env: Env,
        smart_account: Address,
        fee_token: Address,
        amount: i128,
    ) -> Result<(), WalletError> {
        let relayer = Self::get_relayer(env.clone())?;
        relayer.require_auth();

        let accepted_tokens = Self::get_accepted_tokens(env.clone());
        if !accepted_tokens.contains(&fee_token) {
            return Err(WalletError::Unauthorized); // Token not accepted
        }

        // Call the SAC (Stellar Asset Contract) transfer to pull fee token to the paymaster address
        env.invoke_contract::<()>(
            &fee_token,
            &symbol_short!("transfer"),
            (
                smart_account.clone(),
                env.current_contract_address(),
                amount,
            )
                .into_val(&env),
        );

        env.events().publish(
            (Symbol::new(&env, "fee_collected"),),
            (smart_account, fee_token, amount),
        );

        Ok(())
    }

    /// Reclaim collected fee tokens (e.g. convert to XLM or transfer to admin). Requires admin auth.
    pub fn reclaim_fees(
        env: Env,
        token: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), WalletError> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        env.invoke_contract::<()>(
            &token,
            &symbol_short!("transfer"),
            (
                env.current_contract_address(),
                to,
                amount,
            )
                .into_val(&env),
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
}
