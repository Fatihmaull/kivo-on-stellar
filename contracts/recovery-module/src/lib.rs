#![no_std]

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, IntoVal, Symbol, Vec};

use novus_types::{ProposalStatus, RecoveryDataKey, RecoveryProposal, WalletConfig, WalletError};

/// ═══════════════════════════════════════════════════════════════════════
/// RecoveryModule Contract
/// ═══════════════════════════════════════════════════════════════════════
///
/// m-of-n guardian social recovery for a single bound SmartAccount. A
/// proposal's timelock expiry is stored directly on the `RecoveryProposal`
/// (Persistent) rather than in a separate Temporary entry — a Temporary
/// deadline that gets archived would strand the proposal at
/// `TimelockStarted` forever, since nothing could ever read the expiry
/// again to let `execute_recovery` proceed.
#[contract]
pub struct RecoveryModuleContract;

#[contractimpl]
impl RecoveryModuleContract {
    pub fn __constructor(env: Env, smart_account: Address) {
        env.storage()
            .instance()
            .set(&RecoveryDataKey::SmartAccount, &smart_account);
        // This module is invoked rarely — often not at all for years — so
        // its own Instance entry (the bound SmartAccount address) needs a
        // deliberately generous TTL rather than the host's small default.
        // Every state-changing call below re-bumps it, the same piggyback
        // pattern the wallet itself uses.
        env.storage().instance().extend_ttl(518_400, 518_400); // ~30 days
    }

    /// Return the bound SmartAccount address.
    pub fn get_smart_account(env: Env) -> Result<Address, WalletError> {
        env.storage()
            .instance()
            .get(&RecoveryDataKey::SmartAccount)
            .ok_or(WalletError::NotInitialized)
    }

    /// Propose a credential rotation. Must be called and authorized by a registered guardian.
    pub fn propose_recovery(
        env: Env,
        guardian: Address,
        new_credential_id: BytesN<32>,
        new_public_key: BytesN<65>,
    ) -> Result<u32, WalletError> {
        guardian.require_auth();

        let smart_account = Self::get_smart_account(env.clone())?;

        let is_g: bool = env.invoke_contract(
            &smart_account,
            &Symbol::new(&env, "is_guardian"),
            (guardian.clone(),).into_val(&env),
        );
        if !is_g {
            return Err(WalletError::Unauthorized);
        }

        let proposal_id = get_next_proposal_id(&env);

        let mut approvals = Vec::new(&env);
        approvals.push_back(guardian);

        let proposal = RecoveryProposal {
            id: proposal_id,
            new_credential_id,
            new_public_key,
            approvals,
            created_at_ledger: env.ledger().sequence(),
            timelock_expires_at: 0,
            status: ProposalStatus::Pending,
        };

        set_proposal(&env, proposal_id, &proposal);

        env.events().publish(
            (Symbol::new(&env, "recovery_proposed"),),
            (proposal_id, smart_account),
        );

        Ok(proposal_id)
    }

    /// Approve an active recovery proposal. Must be called and authorized by another guardian.
    pub fn approve_recovery(
        env: Env,
        guardian: Address,
        proposal_id: u32,
    ) -> Result<ProposalStatus, WalletError> {
        guardian.require_auth();

        let smart_account = Self::get_smart_account(env.clone())?;

        let is_g: bool = env.invoke_contract(
            &smart_account,
            &Symbol::new(&env, "is_guardian"),
            (guardian.clone(),).into_val(&env),
        );
        if !is_g {
            return Err(WalletError::Unauthorized);
        }

        let mut proposal = Self::get_proposal(env.clone(), proposal_id)?;
        if proposal.status != ProposalStatus::Pending {
            return Err(WalletError::ProposalAlreadyExecuted);
        }

        let mut already_approved = false;
        for app in proposal.approvals.iter() {
            if app == guardian {
                already_approved = true;
                break;
            }
        }
        if !already_approved {
            proposal.approvals.push_back(guardian);
        }

        let config: WalletConfig = env.invoke_contract(
            &smart_account,
            &Symbol::new(&env, "get_config"),
            ().into_val(&env),
        );

        if proposal.approvals.len() >= config.recovery_threshold {
            proposal.status = ProposalStatus::TimelockStarted;
            proposal.timelock_expires_at =
                env.ledger().sequence() + config.recovery_timelock_ledgers;

            env.events().publish(
                (Symbol::new(&env, "recovery_locked"),),
                (proposal_id, proposal.timelock_expires_at),
            );
        }

        set_proposal(&env, proposal_id, &proposal);

        Ok(proposal.status)
    }

    /// Execute the credential rotation after the recovery timelock has expired.
    /// Can be called permissionlessly by anyone once timelock criteria is satisfied.
    pub fn execute_recovery(env: Env, proposal_id: u32) -> Result<(), WalletError> {
        let smart_account = Self::get_smart_account(env.clone())?;
        let mut proposal = Self::get_proposal(env.clone(), proposal_id)?;

        if proposal.status != ProposalStatus::TimelockStarted {
            return Err(WalletError::RecoveryNotReady);
        }
        if env.ledger().sequence() < proposal.timelock_expires_at {
            return Err(WalletError::TimelockActive);
        }

        env.invoke_contract::<()>(
            &smart_account,
            &Symbol::new(&env, "rotate_credentials"),
            (
                proposal.new_credential_id.clone(),
                proposal.new_public_key.clone(),
            )
                .into_val(&env),
        );

        proposal.status = ProposalStatus::Executed;
        set_proposal(&env, proposal_id, &proposal);

        env.events().publish(
            (Symbol::new(&env, "recovery_executed"),),
            (proposal_id, smart_account),
        );

        Ok(())
    }

    /// Cancel a recovery proposal. Must be authorized by the wallet itself
    /// (proving the owner still controls the credentials or has regained control).
    pub fn cancel_recovery(env: Env, owner: Address, proposal_id: u32) -> Result<(), WalletError> {
        let smart_account = Self::get_smart_account(env.clone())?;
        if owner != smart_account {
            return Err(WalletError::Unauthorized);
        }
        owner.require_auth();

        let mut proposal = Self::get_proposal(env.clone(), proposal_id)?;
        if proposal.status != ProposalStatus::Pending
            && proposal.status != ProposalStatus::TimelockStarted
        {
            return Err(WalletError::ProposalAlreadyExecuted);
        }

        proposal.status = ProposalStatus::Cancelled;
        set_proposal(&env, proposal_id, &proposal);

        env.events().publish(
            (Symbol::new(&env, "recovery_cancelled"),),
            (proposal_id, smart_account),
        );

        Ok(())
    }

    /// Read a recovery proposal by ID.
    pub fn get_proposal(env: Env, proposal_id: u32) -> Result<RecoveryProposal, WalletError> {
        env.storage()
            .persistent()
            .get(&RecoveryDataKey::Proposal(proposal_id))
            .ok_or(WalletError::ProposalNotFound)
    }
}

fn set_proposal(env: &Env, proposal_id: u32, proposal: &RecoveryProposal) {
    let key = RecoveryDataKey::Proposal(proposal_id);
    env.storage().persistent().set(&key, proposal);
    // Recovery proposals must outlive long dormancy periods just like
    // guardians do — bump generously on every write.
    env.storage()
        .persistent()
        .extend_ttl(&key, 120_960, 518_400);
    // Piggyback refresh for this module's own Instance entry, same as the
    // wallet does for itself on every successful auth.
    env.storage().instance().extend_ttl(120_960, 518_400);
}

#[cfg(test)]
mod test;

/// Helper to get and increment proposal counter
fn get_next_proposal_id(env: &Env) -> u32 {
    let id: u32 = env
        .storage()
        .persistent()
        .get(&RecoveryDataKey::NextProposalId)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&RecoveryDataKey::NextProposalId, &(id + 1));
    id
}
