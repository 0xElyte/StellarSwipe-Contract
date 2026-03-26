#![cfg(test)]

use super::*;
use crate::risk;
use crate::storage;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    Env,
};

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);
    env
}

fn setup_signal(_env: &Env, signal_id: u64, expiry: u64) -> storage::Signal {
    storage::Signal {
        signal_id,
        price: 100,
        expiry,
        base_asset: 1,
    }
}

#[test]
fn test_execute_trade_invalid_amount() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let res =
            AutoTradeContract::execute_trade(env.clone(), user.clone(), 1, OrderType::Market, 0);

        assert_eq!(res, Err(AutoTradeError::InvalidAmount));
    });
}

#[test]
fn test_execute_trade_signal_not_found() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            999,
            OrderType::Market,
            100,
        );

        assert_eq!(res, Err(AutoTradeError::SignalNotFound));
    });
}

#[test]
fn test_execute_trade_signal_expired() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() - 1);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100,
        );

        assert_eq!(res, Err(AutoTradeError::SignalExpired));
    });
}

#[test]
fn test_execute_trade_unauthorized() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100,
        );

        assert_eq!(res, Err(AutoTradeError::Unauthorized));
    });
}

#[test]
fn test_execute_trade_insufficient_balance() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &50i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100,
        );

        assert_eq!(res, Err(AutoTradeError::InsufficientBalance));
    });
}

#[test]
fn test_execute_trade_market_full_fill() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &500i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400,
        )
        .unwrap();

        assert_eq!(res.trade.executed_amount, 400);
        assert_eq!(res.trade.executed_price, 100);
        assert_eq!(res.trade.status, TradeStatus::Filled);
    });
}

#[test]
fn test_execute_trade_market_partial_fill() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 2;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &100i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            300,
        )
        .unwrap();

        assert_eq!(res.trade.executed_amount, 100);
        assert_eq!(res.trade.executed_price, 100);
        assert_eq!(res.trade.status, TradeStatus::PartiallyFilled);
    });
}

#[test]
fn test_execute_trade_limit_filled() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 3;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("price"), signal_id), &90i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Limit,
            200,
        )
        .unwrap();

        assert_eq!(res.trade.executed_amount, 200);
        assert_eq!(res.trade.executed_price, 100);
        assert_eq!(res.trade.status, TradeStatus::Filled);
    });
}

#[test]
fn test_execute_trade_limit_not_filled() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 4;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("price"), signal_id), &150i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Limit,
            200,
        )
        .unwrap();

        assert_eq!(res.trade.executed_amount, 0);
        assert_eq!(res.trade.executed_price, 0);
        assert_eq!(res.trade.status, TradeStatus::Failed);
    });
}

#[test]
fn test_get_trade_existing() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &500i128);
    });

    env.as_contract(&contract_id, || {
        let _ = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400,
        )
        .unwrap();
    });

    env.as_contract(&contract_id, || {
        let trade = AutoTradeContract::get_trade(env.clone(), user.clone(), signal_id).unwrap();

        assert_eq!(trade.executed_amount, 400);
    });
}

#[test]
fn test_get_trade_non_existing() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 999;

    env.as_contract(&contract_id, || {
        let trade = AutoTradeContract::get_trade(env.clone(), user.clone(), signal_id);

        assert!(trade.is_none());
    });
}

// ========================================
// Risk Management Tests
// ========================================

#[test]
fn test_get_default_risk_config() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let config = AutoTradeContract::get_risk_config(env.clone(), user.clone());

        assert_eq!(config.max_position_pct, 20);
        assert_eq!(config.daily_trade_limit, 10);
        assert_eq!(config.stop_loss_pct, 15);
    });
}

#[test]
fn test_set_custom_risk_config() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let custom_config = risk::RiskConfig {
            max_position_pct: 30,
            daily_trade_limit: 15,
            stop_loss_pct: 10,
        };

        AutoTradeContract::set_risk_config(env.clone(), user.clone(), custom_config.clone());

        let retrieved = AutoTradeContract::get_risk_config(env.clone(), user.clone());
        assert_eq!(retrieved, custom_config);
    });
}

#[test]
fn test_position_limit_allows_first_trade() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &1000i128);

        // First trade should be allowed
        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            1000,
        );

        assert!(res.is_ok());
    });
}

#[test]
fn test_get_user_positions() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &500i128);

        // Execute a trade
        let _ = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400,
        )
        .unwrap();

        // Check positions
        let positions = AutoTradeContract::get_user_positions(env.clone(), user.clone());
        assert!(positions.contains_key(1));

        let position = positions.get(1).unwrap();
        assert_eq!(position.amount, 400);
        assert_eq!(position.entry_price, 100);
    });
}

#[test]
fn test_stop_loss_check() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Setup a position with entry price 100
        risk::update_position(&env, &user, 1, 1000, 100);

        let config = risk::RiskConfig::default(); // 15% stop loss

        // Price at 90 (10% drop) - should NOT trigger
        let triggered = risk::check_stop_loss(&env, &user, 1, 90, &config);
        assert!(!triggered);

        // Price at 80 (20% drop) - should trigger
        let triggered = risk::check_stop_loss(&env, &user, 1, 80, &config);
        assert!(triggered);
    });
}

#[test]
fn test_get_trade_history_paginated() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    // Setup (max_position_pct: 100 so multiple buys in same asset pass risk checks)
    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        risk::set_risk_config(
            &env,
            &user,
            &risk::RiskConfig {
                max_position_pct: 100,
                daily_trade_limit: 10,
                stop_loss_pct: 15,
            },
        );
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &5000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &5000i128);
    });

    // Execute 5 trades in separate frames (avoids "frame is already authorized")
    for _ in 0..5 {
        env.as_contract(&contract_id, || {
            let _ = AutoTradeContract::execute_trade(
                env.clone(),
                user.clone(),
                signal_id,
                OrderType::Market,
                100,
            )
            .unwrap();
        });
    }

    // Query history (no auth required)
    env.as_contract(&contract_id, || {
        let history = AutoTradeContract::get_trade_history(env.clone(), user.clone(), 0, 10);
        assert_eq!(history.len(), 5);

        let page2 = AutoTradeContract::get_trade_history(env.clone(), user.clone(), 2, 2);
        assert_eq!(page2.len(), 2);
    });
}

#[test]
fn test_get_trade_history_empty() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let history = AutoTradeContract::get_trade_history(env.clone(), user.clone(), 0, 20);
        assert_eq!(history.len(), 0);
    });
}

#[test]
fn test_get_portfolio() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &500i128);

        let _ = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400,
        )
        .unwrap();

        let portfolio = AutoTradeContract::get_portfolio(env.clone(), user.clone());
        assert_eq!(portfolio.assets.len(), 1);
        assert_eq!(portfolio.assets.get(0).unwrap().amount, 400);
        assert_eq!(portfolio.assets.get(0).unwrap().asset_id, 1);
    });
}

#[test]
fn test_portfolio_value_calculation() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Set up positions and prices
        risk::set_asset_price(&env, 1, 100);
        risk::set_asset_price(&env, 2, 200);

        risk::update_position(&env, &user, 1, 1000, 100);
        risk::update_position(&env, &user, 2, 500, 200);

        let total_value = risk::calculate_portfolio_value(&env, &user);
        // (1000 * 100 / 100) + (500 * 200 / 100) = 1000 + 1000 = 2000
        assert_eq!(total_value, 2000);
    });
}

// ========================================
// Authorization Tests
// ========================================

#[test]
fn test_grant_authorization_success() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let res = AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30);
        assert!(res.is_ok());

        let config = AutoTradeContract::get_auth_config(env.clone(), user.clone()).unwrap();
        assert_eq!(config.authorized, true);
        assert_eq!(config.max_trade_amount, 500_0000000);
        assert_eq!(config.expires_at, 1000 + (30 * 86400));
    });
}

#[test]
fn test_grant_authorization_zero_amount() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let res = AutoTradeContract::grant_authorization(env.clone(), user.clone(), 0, 30);
        assert_eq!(res, Err(AutoTradeError::InvalidAmount));
    });
}

#[test]
fn test_revoke_authorization() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 1000_0000000, 30)
            .unwrap();
        AutoTradeContract::revoke_authorization(env.clone(), user.clone()).unwrap();

        let config = AutoTradeContract::get_auth_config(env.clone(), user.clone());
        assert!(config.is_none());
    });
}

#[test]
fn test_trade_under_limit_succeeds() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30)
            .unwrap();
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000_0000000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &1000_0000000i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400_0000000,
        );
        assert!(res.is_ok());
    });
}

#[test]
fn test_trade_over_limit_fails() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30)
            .unwrap();
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000_0000000i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            600_0000000,
        );
        assert_eq!(res, Err(AutoTradeError::Unauthorized));
    });
}

#[test]
fn test_revoked_authorization_blocks_trade() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 1000_0000000, 30)
            .unwrap();
        AutoTradeContract::revoke_authorization(env.clone(), user.clone()).unwrap();

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100_0000000,
        );
        assert_eq!(res, Err(AutoTradeError::Unauthorized));
    });
}

#[test]
fn test_expired_authorization_blocks_trade() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 100000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        // Grant with 1 day duration
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 1000_0000000, 1)
            .unwrap();

        // Fast forward time beyond expiry
        env.ledger().set_timestamp(1000 + 86400 + 1);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100_0000000,
        );
        assert_eq!(res, Err(AutoTradeError::Unauthorized));
    });
}

#[test]
fn test_multiple_authorization_grants_latest_applies() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30)
            .unwrap();
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 1000_0000000, 60)
            .unwrap();

        let config = AutoTradeContract::get_auth_config(env.clone(), user.clone()).unwrap();
        assert_eq!(config.max_trade_amount, 1000_0000000);
        assert_eq!(config.expires_at, 1000 + (60 * 86400));
    });
}

#[test]
fn test_authorization_at_exact_limit() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30)
            .unwrap();
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000_0000000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &1000_0000000i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            500_0000000,
        );
        assert!(res.is_ok());
    });
}

// ========================================
// DCA Strategy Tests
// ========================================

#[cfg(test)]
mod dca_tests {
    use crate::strategies::dca::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Ledger as _},
        Env,
    };

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_000);
        let user = soroban_sdk::Address::generate(&env);
        (env, user)
    }

    fn set_price(env: &Env, asset: u32, price: i128) {
        env.storage()
            .temporary()
            .set(&(symbol_short!("price"), asset), &price);
    }

    fn set_balance(env: &Env, user: &soroban_sdk::Address, bal: i128) {
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &bal);
    }

    #[test]
    fn test_create_dca_strategy() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, Some(30))
                .unwrap();
            assert_eq!(id, 0);
            let s = get_dca_strategy(&env, id).unwrap();
            assert_eq!(s.purchase_amount, 10);
            assert_eq!(s.status, DCAStatus::Active);
            assert_eq!(s.end_time, 1_000 + 30 * 86_400);
        });
    }

    #[test]
    fn test_first_purchase_executes_immediately() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 1_000);
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, None)
                .unwrap();
            assert!(is_purchase_due(&env, id).unwrap());
            execute_dca_purchase(&env, id).unwrap();
            let s = get_dca_strategy(&env, id).unwrap();
            assert_eq!(s.purchases.len(), 1);
            assert_eq!(s.total_invested, 10);
        });
    }

    #[test]
    fn test_second_purchase_after_one_day() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 1_000);
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, None)
                .unwrap();
            execute_dca_purchase(&env, id).unwrap();

            // Not due yet
            assert!(!is_purchase_due(&env, id).unwrap());

            // Advance 1 day
            env.ledger().set_timestamp(1_000 + 86_400);
            assert!(is_purchase_due(&env, id).unwrap());
            execute_dca_purchase(&env, id).unwrap();

            let s = get_dca_strategy(&env, id).unwrap();
            assert_eq!(s.purchases.len(), 2);
        });
    }

    #[test]
    fn test_average_entry_price_calculation() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_balance(&env, &user, 10_000);
            let id = create_dca_strategy(&env, user.clone(), 1, 100, DCAFrequency::Daily, None)
                .unwrap();

            // Purchase 1 at price 100
            set_price(&env, 1, 100);
            execute_dca_purchase(&env, id).unwrap();

            // Purchase 2 at price 200
            env.ledger().set_timestamp(1_000 + 86_400);
            set_price(&env, 1, 200);
            execute_dca_purchase(&env, id).unwrap();

            let s = get_dca_strategy(&env, id).unwrap();
            // total_invested = 200, total_acquired = 1_000_000 + 500_000 = 1_500_000 (PRECISION=1_000_000)
            // avg = (200 * 1_000_000) / 1_500_000 = 133
            assert!(s.average_entry_price > 0);
            assert!(s.average_entry_price < 200);
        });
    }

    #[test]
    fn test_pause_stops_purchases() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 1_000);
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, None)
                .unwrap();
            execute_dca_purchase(&env, id).unwrap();
            pause_dca_strategy(&env, id).unwrap();

            env.ledger().set_timestamp(1_000 + 86_400);
            assert!(!is_purchase_due(&env, id).unwrap());
        });
    }

    #[test]
    fn test_resume_restarts_purchases() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 1_000);
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, None)
                .unwrap();
            execute_dca_purchase(&env, id).unwrap();
            pause_dca_strategy(&env, id).unwrap();

            env.ledger().set_timestamp(1_000 + 86_400);
            assert!(!is_purchase_due(&env, id).unwrap());

            resume_dca_strategy(&env, id).unwrap();
            assert!(is_purchase_due(&env, id).unwrap());
            execute_dca_purchase(&env, id).unwrap();

            let s = get_dca_strategy(&env, id).unwrap();
            assert_eq!(s.purchases.len(), 2);
        });
    }

    #[test]
    fn test_analyze_performance() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 1_000);
            let id = create_dca_strategy(&env, user.clone(), 1, 100, DCAFrequency::Daily, None)
                .unwrap();
            execute_dca_purchase(&env, id).unwrap();

            let perf = analyze_dca_performance(&env, id).unwrap();
            assert_eq!(perf.total_invested, 100);
            assert_eq!(perf.total_purchases, 1);
            assert_eq!(perf.current_price, 100);
        });
    }

    #[test]
    fn test_end_time_stops_purchases() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 1_000);
            // 1-day duration
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, Some(1))
                .unwrap();
            execute_dca_purchase(&env, id).unwrap();

            // Advance past end_time
            env.ledger().set_timestamp(1_000 + 86_400 + 1);
            assert!(!is_purchase_due(&env, id).unwrap());
        });
    }

    #[test]
    fn test_insufficient_balance_pauses_strategy() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 5); // less than purchase_amount=10
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, None)
                .unwrap();
            let err = execute_dca_purchase(&env, id).unwrap_err();
            assert_eq!(err, crate::errors::AutoTradeError::InsufficientBalance);
            let s = get_dca_strategy(&env, id).unwrap();
            assert_eq!(s.status, DCAStatus::Paused);
        });
    }

    #[test]
    fn test_update_dca_schedule() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, None)
                .unwrap();
            update_dca_schedule(&env, id, Some(50), Some(DCAFrequency::Weekly)).unwrap();
            let s = get_dca_strategy(&env, id).unwrap();
            assert_eq!(s.purchase_amount, 50);
            assert_eq!(s.frequency, DCAFrequency::Weekly);
        });
    }

    #[test]
    fn test_handle_missed_purchases() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 10_000);
            let id = create_dca_strategy(&env, user.clone(), 1, 10, DCAFrequency::Daily, None)
                .unwrap();

            // Advance 3 days without executing
            env.ledger().set_timestamp(1_000 + 3 * 86_400);
            let missed = handle_missed_dca_purchases(&env, id).unwrap();
            assert_eq!(missed, 3);
            let s = get_dca_strategy(&env, id).unwrap();
            assert_eq!(s.purchases.len(), 3);
        });
    }

    #[test]
    fn test_custom_frequency() {
        let (env, user) = setup();
        let contract = env.register(crate::AutoTradeContract, ());
        env.as_contract(&contract, || {
            set_price(&env, 1, 100);
            set_balance(&env, &user, 1_000);
            let id = create_dca_strategy(
                &env,
                user.clone(),
                1,
                10,
                DCAFrequency::Custom { interval_seconds: 3_600 },
                None,
            )
            .unwrap();
            execute_dca_purchase(&env, id).unwrap();

            // Not due after 30 min
            env.ledger().set_timestamp(1_000 + 1_800);
            assert!(!is_purchase_due(&env, id).unwrap());

            // Due after 1 hour
            env.ledger().set_timestamp(1_000 + 3_600);
            assert!(is_purchase_due(&env, id).unwrap());
        });
    }
}
