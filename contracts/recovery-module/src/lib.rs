#![no_std]

//! NovusWallet Recovery Module Contract
//!
//! Manages m-of-n guardian-based social recovery for SmartAccount credential rotation.
//! Implementation deferred to Milestone 2.
//!
//! # Key Functions (M2)
//! - `propose_recovery` - Guardian initiates credential rotation
//! - `approve_recovery` - Guardians endorse a proposal
//! - `execute_recovery` - Permissionless execution after timelock
//! - `cancel_recovery` - Owner vetoes during timelock window

use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct RecoveryModuleContract;

#[contractimpl]
impl RecoveryModuleContract {
    /// Placeholder — full implementation in Milestone 2.
    pub fn version(_env: Env) -> u32 {
        0
    }
}
