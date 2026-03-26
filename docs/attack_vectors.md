# StellarSwipe Attack Vector Analysis

**Version:** 1.0.0  
**Date:** 2025  
**Scope:** `signal_registry`, `auto_trade`, `oracle`, `common`

---

## Overview

This document enumerates potential attack vectors against the StellarSwipe Soroban contracts, assesses their likelihood and impact, and documents the mitigations in place or required before mainnet launch.

**Risk Rating Matrix:**

| Likelihood \ Impact | Low | Medium | High | Critical |
|---|---|---|---|---|
| High | Medium | High | Critical | Critical |
| Medium | Low | Medium | High | Critical |
| Low | Info | Low | Medium | High |

---

## AV-01: Front-Running Signal Submissions

**Category:** MEV / Information Asymmetry  
**Likelihood:** Medium  
**Impact:** Medium  
**Overall Risk:** Medium

### Description

Stellar transactions are publicly visible in the mempool before they are included in a ledger. An attacker (or bot) observing a pending `create_signal` transaction can extract the `asset_pair`, `action` (Buy/Sell), and `price` fields and act on this information before the signal is finalized on-chain.

### Affected Functions

- `SignalRegistry::create_signal`
- `SignalRegistry::create_signal_internal`

### Current Mitigations

- Stellar's ledger closes approximately every 5 seconds, limiting the front-running window.
- Signal price is a target/reference price, not an execution price; acting on it does not guarantee profit.

### Recommended Mitigations

1. **Commit-Reveal Scheme**: Submit a hash of `(provider, asset_pair, action, price, nonce)` in a first transaction, then reveal the full signal in a second transaction after the commit is finalized.
2. **Accept as Known Risk**: For a copy-trading platform, signal content is intended to be public after submission. Front-running the submission window (5s) provides minimal advantage over copying the signal post-publication.

### Residual Risk

Accepted as known risk for MVP. Commit-reveal should be evaluated for high-value signal providers.

---

## AV-02: Oracle Price Manipulation

**Category:** Oracle Attack  
**Likelihood:** Medium  
**Impact:** High  
**Overall Risk:** High

### Description

An attacker who controls one or more registered oracle addresses can submit manipulated prices to trigger stop-losses, prevent valid trades, or cause incorrect ROI calculations.

### Affected Functions

- `OracleContract::submit_price`
- `OracleContract::get_price_with_confidence`
- `OracleContract::calculate_consensus`
- `AutoTradeContract::execute_trade` (via `risk::check_stop_loss`)

### Attack Scenarios

**Scenario A — Single Oracle Manipulation:**  
If only one oracle is registered, a compromised oracle can set any price. The `get_price_with_confidence` function requires at least one fresh price; with a single oracle, there is no cross-validation.

**Scenario B — Coordinated Oracle Manipulation:**  
If an attacker controls a majority of registered oracles, they can submit a coordinated false price that passes the 10% deviation check (all submissions agree on the false price).

**Scenario C — Stale Price Exploitation:**  
If all oracles go offline, prices become stale (>300s). The contract returns `StalePrice` error, blocking all price-dependent operations. This is a denial-of-service vector.

### Current Mitigations

- **10% deviation rejection**: `get_price_with_confidence` rejects prices where `(max - min) / min > 10%`. Prevents single outlier manipulation.
- **Weighted median**: `calculate_consensus` uses a weighted median, making manipulation require control of >50% of weighted oracle votes.
- **Reputation slashing**: Oracles deviating >20% from consensus are slashed and may be removed.
- **Minimum 2 oracles**: `calculate_consensus` requires at least 2 submissions.
- **Price staleness TTL**: Prices expire after 300 seconds, preventing use of outdated data.

### Recommended Mitigations

1. **Minimum oracle count of 3**: Enforce at least 3 oracles for consensus to prevent 2-of-2 collusion.
2. **TWAP sanity check**: Before accepting a spot price, verify it does not deviate more than X% from the 1-hour TWAP.
3. **Circuit breaker**: If consensus price changes by >15% in a single round, pause trading automatically.
4. **External oracle integration**: Integrate Band Protocol or Pyth as an independent price reference.

### Residual Risk

Medium — weighted median and slashing provide meaningful protection against individual oracle compromise. Coordinated multi-oracle attacks remain a risk without external oracle integration.

---

## AV-03: Stake Griefing / Signal Spam

**Category:** Economic Attack / Spam  
**Likelihood:** High  
**Impact:** Medium  
**Overall Risk:** High

### Description

A provider with the minimum stake (100 XLM ≈ ~$10 at current prices) can submit an unlimited number of signals. This can:
1. Flood the signal feed, degrading UX for legitimate users.
2. Dilute leaderboard metrics by submitting many low-quality signals.
3. Exhaust contract storage (instance storage has size limits).

### Affected Functions

- `SignalRegistry::create_signal`
- `SignalRegistry::get_active_signals`
- `SignalRegistry::get_leaderboard`

### Current Mitigations

- Minimum stake of 100 XLM creates a financial barrier.
- 7-day unstake lock after signal submission creates a time cost.
- Signal expiry (max 30 days) limits storage growth over time.

### Recommended Mitigations

1. **Per-provider rate limit**: Maximum N signals per time window (e.g., 10 signals per hour per provider).
2. **Stake-proportional signal limit**: Allow more signals for higher-staked providers.
3. **Reputation-gated submission**: Require minimum reputation score after first 5 signals.
4. **Increase minimum stake**: Raise to 1,000 XLM for mainnet to increase attack cost.

### Residual Risk

High for MVP — no rate limiting is currently implemented. Must be addressed before mainnet.

---

## AV-04: Flash Loan Leaderboard Manipulation

**Category:** Economic Attack  
**Likelihood:** Low  
**Impact:** Medium  
**Overall Risk:** Low

### Description

An attacker could theoretically borrow large amounts of capital, execute many trades through `AutoTradeContract` to inflate their volume metrics, then repay the loan. This would artificially boost their leaderboard ranking by `Volume` metric.

### Affected Functions

- `AutoTradeContract::execute_trade`
- `SignalRegistry::record_trade_execution`
- `SignalRegistry::get_leaderboard` (Volume metric)

### Current Mitigations

- Stellar does not natively support flash loans (no atomic borrow-use-repay in a single transaction).
- Daily trade limit (`daily_trade_limit` in `RiskConfig`) caps trades per user per day.
- Leaderboard requires minimum 5 terminal-status signals, preventing instant manipulation.

### Recommended Mitigations

1. **Time-weighted volume**: Weight volume metrics by position duration rather than raw volume.
2. **Minimum position duration**: Require positions to be held for at least N ledgers before counting toward stats.
3. **Success rate primary metric**: Prioritize `SuccessRate` over `Volume` in default leaderboard display.

### Residual Risk

Low — Stellar's lack of flash loan infrastructure makes this attack impractical in the current ecosystem.

---

## AV-05: Admin Key Compromise

**Category:** Privilege Escalation  
**Likelihood:** Low  
**Impact:** Critical  
**Overall Risk:** High

### Description

If the admin private key is compromised, an attacker can:
1. Transfer admin to their own address via `transfer_admin`.
2. Pause trading indefinitely (48-hour auto-expiry limits this).
3. Set malicious fee rates (capped at 1% by `MAX_FEE_BPS`).
4. Set extreme risk parameters (stop_loss = 100%, position_limit = 100%).
5. Register malicious oracles.

### Affected Functions

- `SignalRegistry::transfer_admin`
- `SignalRegistry::pause_trading`
- `SignalRegistry::set_trade_fee`
- `SignalRegistry::set_risk_defaults`
- `OracleContract::register_oracle`

### Current Mitigations

- Multi-sig admin module is implemented and can be enabled post-deploy.
- Fee rate is capped at 1% (`MAX_FEE_BPS = 100`).
- Pause auto-expires after 48 hours.

### Recommended Mitigations

1. **Enable multi-sig before mainnet**: Call `enable_multisig` with a 2-of-3 or 3-of-5 signer set.
2. **Two-step admin transfer**: Add a pending admin state that requires acceptance from the new admin address.
3. **Time-lock on critical parameter changes**: Add a 24-hour delay before fee/risk parameter changes take effect.
4. **Hardware wallet for admin key**: Use a hardware security module for the admin signing key.

### Residual Risk

Medium with multi-sig enabled. Critical without it. Multi-sig must be enabled before mainnet launch.

---

## AV-06: Collaborative Signal Approval Bypass

**Category:** Logic Error  
**Likelihood:** Low  
**Impact:** Medium  
**Overall Risk:** Low

### Description

In the collaborative signal flow, a signal starts in `Pending` status and transitions to `Active` only when all co-authors approve. An attacker who is a co-author could:
1. Approve their own signal immediately after creation.
2. If the primary author is also a co-author, they could self-approve.

### Affected Functions

- `SignalRegistry::create_collaborative_signal`
- `SignalRegistry::approve_collaborative_signal`

### Current Mitigations

- `collaboration::approve_collaborative_signal` tracks per-author approval state.
- `AlreadyApproved` error prevents double-approval.

### Recommended Mitigations

1. **Verify co-author != primary author**: Prevent the primary author from being listed as a co-author.
2. **Minimum co-author count**: Require at least 2 distinct co-authors for collaborative signals.
3. **Approval timeout**: If not all co-authors approve within N days, auto-expire the signal.

### Residual Risk

Low — the approval mechanism is sound; edge cases around self-approval should be explicitly tested.

---

## AV-07: Integer Overflow / Underflow in Financial Calculations

**Category:** Arithmetic Vulnerability  
**Likelihood:** Low  
**Impact:** High  
**Overall Risk:** Medium

### Description

Financial calculations involving multiplication of large `i128` values (prices × amounts) could overflow even with `overflow-checks = true` in release builds (which causes a panic rather than silent wrap-around).

### Affected Code

```rust
// risk.rs — potential overflow
let new_position_value = new_position_amount * trade_price / 100;
let trade_value = trade_amount * trade_price / 100;

// oracle/lib.rs — potential division by zero
let deviation = ((submission.price - consensus_price).abs() * 10000) / consensus_price;

// oracle/lib.rs — potential division by zero
if (max_p - min_p) * 100 / min_p > 10 { ... }
```

### Current Mitigations

- `overflow-checks = true` in `[profile.release]` causes panics on overflow rather than silent wrap.
- Signal ID counter uses `checked_add`.

### Recommended Mitigations

1. **Use `checked_mul` / `checked_div`** for all financial arithmetic instead of relying on panic-on-overflow.
2. **Guard all division operations**: Check divisor != 0 before dividing.
3. **Define maximum price and amount bounds**: Reject inputs exceeding safe multiplication bounds.

### Residual Risk

Medium — panics are safer than silent overflow but still cause transaction failure. Checked arithmetic should be used throughout.

---

## AV-08: Storage Exhaustion (DoS via Storage Growth)

**Category:** Denial of Service  
**Likelihood:** Medium  
**Impact:** Medium  
**Overall Risk:** Medium

### Description

Soroban instance storage has a maximum size per ledger entry. The `Signals` map stored in instance storage grows with every signal submission. If the map grows too large, the contract may fail to read or write it, effectively bricking the contract.

### Affected Storage Keys

- `StorageKey::Signals` — `Map<u64, Signal>` in instance storage
- `StorageKey::ProviderStats` — `Map<Address, ProviderPerformance>` in instance storage

### Current Mitigations

- `cleanup_expired_signals` and `archive_old_signals` functions exist to prune old data.
- Signal expiry (max 30 days) limits the active signal window.

### Recommended Mitigations

1. **Migrate to persistent storage**: Move `Signals` and `ProviderStats` to persistent storage with per-entry keys (e.g., `Signal(u64)` → `Signal`).
2. **Enforce cleanup before submission**: Require a cleanup call if signal count exceeds a threshold.
3. **Hard cap on active signals**: Limit total active signals to a safe maximum (e.g., 10,000).

### Residual Risk

Medium — must be addressed before mainnet with high signal volume.

---

## AV-09: Unauthorized Trade Execution via Expired Authorization

**Category:** Access Control  
**Likelihood:** Low  
**Impact:** High  
**Overall Risk:** Medium

### Description

The `is_authorized` function checks `current_time < cfg.expires_at`. If a user's authorization expires but they do not revoke it, the authorization record remains in storage. A race condition at the exact expiry timestamp could theoretically allow a trade to slip through.

### Affected Functions

- `AutoTradeContract::execute_trade`
- `auth::is_authorized`

### Current Mitigations

- Strict `<` comparison (not `<=`) on expiry timestamp.
- `require_auth()` is called before authorization check, ensuring the user signed the transaction.

### Recommended Mitigations

1. **Auto-cleanup expired authorizations**: Remove expired auth records during the check.
2. **Emit event on authorization expiry**: Allow off-chain monitoring of expired authorizations.

### Residual Risk

Low — the `require_auth()` check ensures the user must sign every transaction regardless of stored authorization.

---

## AV-10: Signal Performance Manipulation via Wash Trading

**Category:** Market Manipulation  
**Likelihood:** Medium  
**Impact:** Medium  
**Overall Risk:** Medium

### Description

A provider can call `record_trade_execution` with their own address as both provider and executor, using fabricated `entry_price` and `exit_price` values to inflate their `success_rate` and `avg_return` metrics on the leaderboard.

### Affected Functions

- `SignalRegistry::record_trade_execution`
- `SignalRegistry::get_leaderboard`

### Current Mitigations

- `executor.require_auth()` ensures the executor signed the transaction (prevents impersonation).
- Entry and exit prices must be positive.

### Recommended Mitigations

1. **Require executor != provider**: Prevent self-reporting of trade executions.
2. **Cross-reference with AutoTradeContract**: Only accept `record_trade_execution` calls from the `AutoTradeContract` address.
3. **Minimum volume threshold**: Ignore executions below a minimum volume for stats purposes.

### Residual Risk

High for leaderboard integrity — wash trading is currently possible and should be addressed before mainnet.

---

## Summary Table

| ID | Attack Vector | Likelihood | Impact | Risk | Status |
|---|---|---|---|---|---|
| AV-01 | Front-running signal submissions | Medium | Medium | Medium | Accepted (known risk) |
| AV-02 | Oracle price manipulation | Medium | High | High | Partially mitigated |
| AV-03 | Stake griefing / signal spam | High | Medium | High | ❌ Not mitigated — rate limiting needed |
| AV-04 | Flash loan leaderboard manipulation | Low | Medium | Low | Mitigated (Stellar architecture) |
| AV-05 | Admin key compromise | Low | Critical | High | Partially mitigated (multi-sig available) |
| AV-06 | Collaborative signal approval bypass | Low | Medium | Low | Partially mitigated |
| AV-07 | Integer overflow in financial math | Low | High | Medium | Partially mitigated (overflow-checks) |
| AV-08 | Storage exhaustion DoS | Medium | Medium | Medium | Partially mitigated (cleanup functions) |
| AV-09 | Expired authorization race condition | Low | High | Medium | Mitigated |
| AV-10 | Wash trading / performance manipulation | Medium | Medium | Medium | ❌ Not mitigated — executor != provider check needed |

### Pre-Mainnet Required Fixes

| Priority | Item | Attack Vector |
|---|---|---|
| P0 | Resolve merge conflicts in `lib.rs` files | All |
| P0 | Implement signal rate limiting per provider | AV-03 |
| P0 | Prevent executor == provider in `record_trade_execution` | AV-10 |
| P1 | Enable multi-sig admin before mainnet | AV-05 |
| P1 | Extend pause check to `AutoTradeContract.execute_trade` | AV-05 |
| P1 | Replace `is_sell = false` hardcode with signal-derived value | AV-02 |
| P1 | Add zero-divisor guards in oracle deviation calculations | AV-07 |
| P2 | Migrate `Signals` map to persistent storage | AV-08 |
| P2 | Add two-step admin transfer | AV-05 |
| P2 | Implement TWAP sanity check on spot prices | AV-02 |
