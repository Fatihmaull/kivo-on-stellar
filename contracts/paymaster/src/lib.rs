#![no_std]

//! NovusWallet Paymaster Contract
//!
//! Manages fee collection in non-XLM tokens and conversion to XLM
//! via the Stellar DEX for relayer reimbursement.
//! Implementation deferred to Milestone 2.
//!
//! # Key Functions (M2)
//! - `initialize` - Set up accepted tokens, relayer address, fee margins
//! - `collect_fee` - Deduct fee tokens from a SmartAccount
//! - `convert_to_xlm` - Swap collected tokens to XLM via DEX path payment

use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct PaymasterContract;

#[contractimpl]
impl PaymasterContract {
    /// Placeholder — full implementation in Milestone 2.
    pub fn version(_env: Env) -> u32 {
        0
    }
}
