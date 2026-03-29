//! Pre-trade safety checks (position caps, etc.).
//!
//! Copy trading consults the configured **user portfolio** contract for open position
//! counts via `get_open_position_count(user)`. Point this at your deployment’s portfolio
//! contract (any Soroban contract that exposes that function and symbol).

use soroban_sdk::{Address, Env, IntoVal, Symbol, Val, Vec};

use crate::errors::ContractError;

/// Default maximum open copy-trade positions per user (safety rail for novices).
pub const MAX_POSITIONS_PER_USER: u32 = 20;

/// Portfolio entrypoint: `get_open_position_count(user: Address) -> u32`.
pub const GET_OPEN_POSITION_COUNT_FN: &str = "get_open_position_count";

/// Enforce per-user open position cap unless `user` is on the admin whitelist.
///
/// Call from [`crate::TradeExecutorContract::execute_copy_trade`] **before** any state
/// changes or downstream portfolio updates.
pub fn check_position_limit(
    env: &Env,
    user_portfolio: &Address,
    user: &Address,
    position_limit_exempt: bool,
) -> Result<(), ContractError> {
    if position_limit_exempt {
        return Ok(());
    }

    let sym = Symbol::new(env, GET_OPEN_POSITION_COUNT_FN);
    let mut args = Vec::<Val>::new(env);
    args.push_back(user.clone().into_val(env));

    let open_count: u32 = env.invoke_contract(user_portfolio, &sym, args);
    if open_count >= MAX_POSITIONS_PER_USER {
        return Err(ContractError::PositionLimitReached);
    }

    Ok(())
}
