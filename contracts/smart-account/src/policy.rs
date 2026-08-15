/// Policy enforcement engine (built-in).
///
/// Enforces spending limits and other constraints during __check_auth.
/// This is the lightweight, built-in policy engine. The external
/// PolicyEngine contract (Milestone 2) will provide more advanced
/// features like per-token limits and contract-level ACLs.

use soroban_sdk::{auth::Context, symbol_short, Env, Symbol, Vec};

use novus_types::WalletError;

use crate::storage::Storage;

/// Number of ledgers in a "day" for spending limit tracking.
/// ~17,280 ledgers × 5s/ledger = ~86,400 seconds = 24 hours
const LEDGERS_PER_DAY: u32 = 17_280;

/// Enforce all configured policies against the auth contexts.
///
/// Currently enforces:
/// - Daily spending limits per token contract
///
/// Future (M2): Delegate to external PolicyEngine for advanced rules.
pub fn enforce_policies(
    env: &Env,
    contexts: &Vec<Context>,
) -> Result<(), WalletError> {
    let config = Storage::get_config(env)?;
    let day_index = current_day_index(env);

    for ctx in contexts.iter() {
        if let Context::Contract(c) = ctx {
            // Only enforce spending limits on token transfer functions
            if is_transfer_function(&c.fn_name) {
                if let Some(amount) = extract_amount(env, &c.args) {
                    // Look up current daily spend for this token
                    let spent = Storage::get_daily_spend(env, &c.contract, day_index);
                    let new_total = spent + amount;

                    if new_total > config.default_daily_limit {
                        return Err(WalletError::SpendingLimitExceeded);
                    }

                    // Record the new spending total
                    Storage::set_daily_spend(env, &c.contract, day_index, new_total);
                }
            }
        }
    }

    Ok(())
}

/// Calculate the current "day index" from the ledger sequence.
/// This creates a rolling 24-hour window for spending limit tracking.
fn current_day_index(env: &Env) -> u32 {
    env.ledger().sequence() / LEDGERS_PER_DAY
}

/// Check if a function name is a token transfer operation.
fn is_transfer_function(fn_name: &Symbol) -> bool {
    let transfer = symbol_short!("transfer");
    *fn_name == transfer
}

/// Extract the amount from transfer function arguments.
/// SAC transfer: transfer(from, to, amount) → amount at index 2
fn extract_amount(env: &Env, args: &soroban_sdk::Vec<soroban_sdk::Val>) -> Option<i128> {
    use soroban_sdk::TryFromVal;
    if args.len() >= 3 {
        i128::try_from_val(env, &args.get(2)?).ok()
    } else {
        None
    }
}
