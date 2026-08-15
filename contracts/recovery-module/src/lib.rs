#![no_std]

use soroban_sdk::{
    contract, contractimpl, Address, BytesN, Env, IntoVal, Symbol, Vec,
};

use novus_types::{
    ProposalStatus, RecoveryDataKey, RecoveryProposal, WalletConfig, WalletError,
};

#[contract]
pub struct RecoveryModuleContract;

#[contractimpl]
impl RecoveryModuleContract {
    /// Initialize the recovery module with the target SmartAccount contract.
    pub fn initialize(env: Env, smart_account: Address) -> Result<(), WalletError> {
        if env.storage().instance().has(&RecoveryDataKey::SmartAccount) {
            return Err(WalletError::AlreadyInitialized);
        }

        env.storage()
            .instance()
            .set(&RecoveryDataKey::SmartAccount, &smart_account);

        Ok(())
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

        // Verify the caller is a registered guardian on the SmartAccount
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
            status: ProposalStatus::Pending,
        };

        env.storage()
            .persistent()
            .set(&RecoveryDataKey::Proposal(proposal_id), &proposal);

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

        // Verify guardian registration
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
            return Err(WalletError::Unauthorized);
        }

        // Add approval if not already approved by this guardian
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

        // Fetch SmartAccount threshold configuration
        let config: WalletConfig = env.invoke_contract(
            &smart_account,
            &Symbol::new(&env, "get_config"),
            ().into_val(&env),
        );

        // Check if recovery threshold is met
        if proposal.approvals.len() >= config.recovery_threshold {
            proposal.status = ProposalStatus::TimelockStarted;

            let expires_at = env.ledger().sequence() + config.recovery_timelock_ledgers;
            env.storage()
                .temporary()
                .set(&RecoveryDataKey::Timelock(proposal_id), &expires_at);

            // Extend temporary storage TTL to safely cover the timelock window
            env.storage().temporary().extend_ttl(
                &RecoveryDataKey::Timelock(proposal_id),
                config.recovery_timelock_ledgers + 100,
                config.recovery_timelock_ledgers + 1000,
            );

            env.events().publish(
                (Symbol::new(&env, "recovery_locked"),),
                (proposal_id, expires_at),
            );
        }

        env.storage()
            .persistent()
            .set(&RecoveryDataKey::Proposal(proposal_id), &proposal);

        Ok(proposal.status)
    }

    /// Execute the credential rotation after the recovery timelock has expired.
    /// Can be called permissionlessly by anyone once timelock criteria is satisfied.
    pub fn execute_recovery(env: Env, proposal_id: u32) -> Result<(), WalletError> {
        let smart_account = Self::get_smart_account(env.clone())?;
        let mut proposal = Self::get_proposal(env.clone(), proposal_id)?;

        if proposal.status != ProposalStatus::TimelockStarted {
            return Err(WalletError::Unauthorized);
        }

        // Load and check the timelock expiration sequence
        let expires_at: u32 = env
            .storage()
            .temporary()
            .get(&RecoveryDataKey::Timelock(proposal_id))
            .ok_or(WalletError::Unauthorized)?;

        if env.ledger().sequence() < expires_at {
            return Err(WalletError::Unauthorized); // Timelock is still active
        }

        // Execute cross-contract rotation on the smart account
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
        env.storage()
            .persistent()
            .set(&RecoveryDataKey::Proposal(proposal_id), &proposal);

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

        // Verifies original owner authentication
        owner.require_auth();

        let mut proposal = Self::get_proposal(env.clone(), proposal_id)?;
        if proposal.status != ProposalStatus::Pending && proposal.status != ProposalStatus::TimelockStarted {
            return Err(WalletError::Unauthorized);
        }

        proposal.status = ProposalStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&RecoveryDataKey::Proposal(proposal_id), &proposal);

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
            .ok_or(WalletError::SignerNotFound) // Map to general error
    }
}

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
