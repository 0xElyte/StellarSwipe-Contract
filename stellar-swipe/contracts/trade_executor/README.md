 feature/position-limit-copy-trade
# Trade executor

## Copy trade position limit (`risk_gates.rs`)

Before opening a new copy position, [`TradeExecutorContract::execute_copy_trade`] calls [`risk_gates::check_position_limit`], which:

1. Returns `Ok(())` if the user is on the admin **position-limit whitelist** (instance key `PositionLimitExempt(user) == true`).
2. Otherwise invokes **`get_open_position_count(user) -> u32`** on the configured **user portfolio** contract via `Env::invoke_contract`.
3. Returns `ContractError::PositionLimitReached` when `open_count >= MAX_POSITIONS_PER_USER` (default **20**).

The check runs **before** `record_copy_position` is invoked on the portfolio, so no executor-side state changes happen when the limit applies.

### Portfolio contract ABI

- `get_open_position_count(user: Address) -> u32` — required for the limit check.
- `record_copy_position(user: Address)` — called after a successful check (void return). Your portfolio contract should persist the new open position here (or equivalent).

### Admin

- `set_user_portfolio` — portfolio contract address.
- `set_position_limit_exempt(user, exempt)` — per-user bypass of the cap.

# Trade executor — SDEX / router integration

This contract swaps Stellar Asset Contracts (SACs) by delegating execution to a **Soroban router** that stands in for classic SDEX path execution (strict-send style fills). There is no single host function that runs the legacy order book from Soroban; production setups use a router (aggregator, pool router, or protocol entrypoint) that performs the path and settles on-chain.

## Invocation pattern (`sdex.rs`)

1. **Approve the router** on the input SAC with `soroban_sdk::token::Client::approve`, authorizing the router to pull `amount` of `from_token` from the executor’s balance (SEP-41).
2. **Call the router** with `Env::invoke_contract(router, Symbol::new(env, "swap"), args)` where `args` is a vector of `Val` in this order:
   - `pull_from`: `Address` — contract whose balance is debited (the executor).
   - `from_token`, `to_token`: input and output SAC addresses.
   - `amount_in`: `i128`.
   - `min_out`: `i128` — router-level minimum; the executor still enforces its own floor.
   - `recipient`: `Address` — where output tokens are credited (usually the same as `pull_from`).
3. **Verify the fill** by comparing the **output token balance delta** on the executor to `min_received`. If `actual_received < min_received`, the helper returns `ContractError::SlippageExceeded` (do not rely only on the router’s return value).

## Slippage helper

For `swap_with_slippage`, minimum output is:

`min_received = amount * (10_000 - max_slippage_bps) / 10_000`

(with `max_slippage_bps >= 10_000` treated as zero minimum at the formula level; invalid `amount` still errors).

## Tests

`src/test.rs` registers a **mock router** that `transfer_from`s the input token and `transfer`s a configurable `amount_out` to the recipient, so you can simulate under-fill and slippage failures without a live SDEX.

**Note:** In tests, configure the mock with `MockSdexRouterClient::set_amount_out` from a **top-level** call. Wrapping that call in `Env::as_contract(&router_id, …)` causes “contract re-entry is not allowed” because the client already invokes the router contract.
 main
