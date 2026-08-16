/// Policy enforcement during __check_auth.
///
/// Two layers, cheapest first:
/// 1. **Built-in daily limit** — always active, enforced directly against
///    `WalletConfig.default_daily_limit`. Zero cross-contract calls.
/// 2. **PolicyEngine** — if the wallet has registered one (`set_policy_engine`),
///    it is invoked for a whitelist + per-token override check on top of
///    the built-in limit. A PolicyEngine violation reverts the whole
///    transaction just like any other check here.
use soroban_sdk::{auth::Context, symbol_short, Env, IntoVal, Symbol, Vec};

use novus_types::WalletError;

use crate::storage::Storage;

/// Number of ledgers in a "day" for spending limit tracking.
/// ~17,280 ledgers × 5s/ledger = ~86,400 seconds = 24 hours
const LEDGERS_PER_DAY: u32 = 17_280;

/// Enforce all configured policies against the auth contexts.
pub fn enforce_policies(env: &Env, contexts: &Vec<Context>) -> Result<(), WalletError> {
    let config = Storage::get_config(env)?;
    let day_index = current_day_index(env);
    let self_address = env.current_contract_address();

    for ctx in contexts.iter() {
        if let Context::Contract(c) = ctx {
            if is_transfer_function(&c.fn_name) {
                if let Some((from, amount)) = extract_transfer(env, &c.args) {
                    // Only an outbound transfer (this wallet is the sender)
                    // consumes daily limit. Without this check, receiving
                    // funds burns down the same budget as sending them.
                    if from == self_address {
                        let spent = Storage::get_daily_spend(env, &c.contract, day_index);
                        let new_total = spent + amount;

                        if new_total > config.default_daily_limit {
                            return Err(WalletError::SpendingLimitExceeded);
                        }

                        Storage::set_daily_spend(env, &c.contract, day_index, new_total);
                    }
                }
            }
        }
    }

    // Optional second layer: delegate to the registered PolicyEngine for
    // whitelist enforcement and per-token limit overrides. A rejection
    // there traps this call and reverts the whole transaction.
    if let Some(policy_engine) = Storage::get_policy_engine(env) {
        env.invoke_contract::<()>(
            &policy_engine,
            &Symbol::new(env, "check_policy"),
            (self_address, contexts.clone()).into_val(env),
        );
    }

    Ok(())
}

fn current_day_index(env: &Env) -> u32 {
    env.ledger().sequence() / LEDGERS_PER_DAY
}

fn is_transfer_function(fn_name: &Symbol) -> bool {
    let transfer = symbol_short!("transfer");
    *fn_name == transfer
}

/// SAC transfer: `transfer(from: Address, to: Address, amount: i128)`
fn extract_transfer(
    env: &Env,
    args: &soroban_sdk::Vec<soroban_sdk::Val>,
) -> Option<(soroban_sdk::Address, i128)> {
    use soroban_sdk::{Address, TryFromVal};
    if args.len() < 3 {
        return None;
    }
    let from = Address::try_from_val(env, &args.get(0)?).ok()?;
    let amount = i128::try_from_val(env, &args.get(2)?).ok()?;
    Some((from, amount))
}
