#![no_std]

mod errors;
feature/position-limit-copy-trade
pub mod risk_gates;

use errors::ContractError;
use risk_gates::check_position_limit;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, IntoVal, Symbol, Val, Vec};

/// Instance storage keys.
#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    Admin,
    /// Contract implementing `get_open_position_count(user) -> u32` (UserPortfolio).
    UserPortfolio,
    /// When set to `true`, this user bypasses [`risk_gates::MAX_POSITIONS_PER_USER`].
    PositionLimitExempt(Address),
}

/// Symbol invoked on the portfolio after a successful limit check (test / integration hook).
pub const RECORD_COPY_POSITION_FN: &str = "record_copy_position";



pub mod sdex;

use errors::ContractError;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

use sdex::{execute_sdex_swap, min_received_from_slippage};

#[contracttype]
#[derive(Clone)]
enum StorageKey {
    Admin,
    SdexRouter,
}

 main
#[contract]
pub struct TradeExecutorContract;

#[contractimpl]
impl TradeExecutorContract {
  feature/position-limit-copy-trade

    /// One-time init; stores admin who may configure the SDEX router address.
main
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&StorageKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&StorageKey::Admin, &admin);
    }

 feature/position-limit-copy-trade
    /// Configure the portfolio contract used for open-position counts and copy-trade recording.
    pub fn set_user_portfolio(env: Env, portfolio: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        env.storage()
            .instance()
            .set(&StorageKey::UserPortfolio, &portfolio);
    }

    pub fn get_user_portfolio(env: Env) -> Option<Address> {
        env.storage().instance().get(&StorageKey::UserPortfolio)
    }

    /// Admin override: exempt `user` from the per-user position cap (or clear exemption).
    pub fn set_position_limit_exempt(env: Env, user: Address, exempt: bool) {

    /// Set the router contract invoked by [`sdex::execute_sdex_swap`].
    pub fn set_sdex_router(env: Env, router: Address) {
 main
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("not initialized");
        admin.require_auth();
feature/position-limit-copy-trade
        let key = StorageKey::PositionLimitExempt(user);
        if exempt {
            env.storage().instance().set(&key, &true);
        } else {
            env.storage().instance().remove(&key);
        }
    }

    pub fn is_position_limit_exempt(env: Env, user: Address) -> bool {
        let key = StorageKey::PositionLimitExempt(user);
        env.storage().instance().get(&key).unwrap_or(false)
    }

    /// Runs copy trade: position limit check first, then portfolio `record_copy_position`.
    pub fn execute_copy_trade(env: Env, user: Address) -> Result<(), ContractError> {
        user.require_auth();

        let portfolio: Address = env
            .storage()
            .instance()
            .get(&StorageKey::UserPortfolio)
            .ok_or(ContractError::NotInitialized)?;

        let exempt = {
            let key = StorageKey::PositionLimitExempt(user.clone());
            env.storage().instance().get(&key).unwrap_or(false)
        };

        check_position_limit(&env, &portfolio, &user, exempt)?;

        let sym = Symbol::new(&env, RECORD_COPY_POSITION_FN);
        let mut args = Vec::<Val>::new(&env);
        args.push_back(user.into_val(&env));
        env.invoke_contract::<()>(&portfolio, &sym, args);

        Ok(())

        env.storage().instance().set(&StorageKey::SdexRouter, &router);
    }

    /// Read configured router (for off-chain tooling).
    pub fn get_sdex_router(env: Env) -> Option<Address> {
        env.storage().instance().get(&StorageKey::SdexRouter)
    }

    /// Swap using a caller-supplied minimum output (already includes slippage tolerance).
    pub fn swap(
        env: Env,
        from_token: Address,
        to_token: Address,
        amount: i128,
        min_received: i128,
    ) -> Result<i128, ContractError> {
        let router = env
            .storage()
            .instance()
            .get(&StorageKey::SdexRouter)
            .ok_or(ContractError::NotInitialized)?;
        execute_sdex_swap(
            &env,
            &router,
            &from_token,
            &to_token,
            amount,
            min_received,
        )
    }

    /// Swap with `min_received = amount * (10000 - max_slippage_bps) / 10000`.
    pub fn swap_with_slippage(
        env: Env,
        from_token: Address,
        to_token: Address,
        amount: i128,
        max_slippage_bps: u32,
    ) -> Result<i128, ContractError> {
        let min_received =
            min_received_from_slippage(amount, max_slippage_bps).ok_or(ContractError::InvalidAmount)?;
        Self::swap(env, from_token, to_token, amount, min_received)
main
    }
}

#[cfg(test)]
mod test;
