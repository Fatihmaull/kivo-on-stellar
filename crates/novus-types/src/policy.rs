use soroban_sdk::contracttype;

/// Result of a policy check performed during __check_auth.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyResult {
    /// Transaction is within all policy bounds
    Approved,
    /// Transaction exceeds daily spending limit
    SpendingLimitExceeded,
    /// Target contract is not whitelisted
    UnauthorizedTarget,
    /// Target function is not whitelisted
    UnauthorizedFunction,
}
