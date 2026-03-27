# StellarSwipe Security Audit Preparation

**Version:** 1.0.0  
**Date:** 2025  
**Status:** Audit-Ready  
**Protocol:** Soroban / Stellar Protocol 23 (Whisk)

---

## 1. Architecture Overview

### 1.1 Contract Inventory

| Contract | Path | Purpose |
|---|---|---|
| `signal_registry` | `stellar-swipe/contracts/signal_registry` | Signal submission, staking, performance tracking, leaderboard |
| `auto_trade` | `stellar-swipe/contracts/auto_trade` | Copy-trade execution on SDEX with risk controls |
| `oracle` | `stellar-swipe/contracts/oracle` | Multi-source price aggregation with TWAP and reputation |
| `common` | `stellar-swipe/contracts/common` | Shared asset types and validation utilities |

### 1.2 Contract Interaction Flow

```
User / Frontend
      │
      ▼
SignalRegistry ──────────────────────────────────────────────────────┐
  • initialize(admin)                                                  │
  • create_signal(provider, asset_pair, action, price, ...)           │
  • record_trade_execution(executor, signal_id, entry, exit, volume)  │
  • get_leaderboard(metric, limit)                                     │
  • pause_trading / unpause_trading (admin)                           │
      │                                                                │
      │  reads signal data                                             │
      ▼                                                                │
AutoTradeContract                                                      │
  • execute_trade(user, signal_id, order_type, amount)                │
  • grant_authorization(user, max_amount, duration_days)              │
  • set_risk_config(user, config)                                      │
  • get_portfolio(user)                                                │
      │                                                                │
      │  price queries                                                 │
      ▼                                                                │
OracleContract                                                         │
  • submit_price(oracle, pair, price, confidence)                     │
  • get_price_with_confidence(pair)                                    │
  • calculate_consensus()                                              │
  • get_twap_1h / get_twap_24h / get_twap_7d(pair)                   │
      │                                                                │
      ▼                                                                │
Stellar SDEX / Liquidity Pools ◄─────────────────────────────────────┘
```

### 1.3 Storage Layout

**SignalRegistry (instance storage):**
- `SignalCounter` → `u64` — monotonically increasing signal ID
- `Signals` → `Map<u64, Signal>` — all signals
- `ProviderStats` → `Map<Address, ProviderPerformance>` — per-provider metrics
- `TemplateCounter` / `Templates` — signal templates
- `ExternalIdMappings` — import deduplication

**SignalRegistry (admin module, instance storage):**
- `Admin` → `Address`
- `MinStake`, `TradeFee`, `StopLoss`, `PositionLimit` → config values
- `PauseInfo` → `{ is_paused, paused_at, expires_at }`
- `MultiSigEnabled`, `MultiSigSigners`, `MultiSigThreshold`

**AutoTradeContract (persistent storage):**
- `Trades(Address, u64)` → `Trade` — per-user per-signal trade record
- `UserRiskConfig(Address)` → `RiskConfig`
- `UserPositions(Address)` → `Map<u32, Position>`
- `UserTradeHistory(Address)` → `Vec<TradeRecord>`

**OracleContract (mixed storage):**
- `PriceMap(AssetPair)` → `Vec<PriceData>` (temporary, TTL 300s)
- `OracleWeight(Address)` → `u32` (persistent)
- `ConsensusPrice` → `ConsensusPriceData` (persistent)
- `Oracles` → `Vec<Address>` (persistent)

### 1.4 Trust Boundaries

```
TRUSTED                          UNTRUSTED
───────────────────────────────────────────────────────
Stellar network consensus        External oracle data sources
Soroban host functions           Signal provider rationale
Admin address (post-deploy)      User-supplied trade amounts
Contract-internal arithmetic     Frontend input parameters
                                 SDEX liquidity availability
```

---

## 2. Security Assumptions

### 2.1 Network-Level Assumptions

- **Stellar consensus is Byzantine-fault-tolerant**: We assume the Stellar network correctly orders and finalizes transactions. No contract-level replay protection is needed beyond Stellar's native sequence numbers.
- **Soroban host is trusted**: All host functions (`env.ledger().timestamp()`, `env.storage()`, `require_auth()`) behave as documented. We do not defend against a malicious host.
- **`require_auth()` is sufficient for authorization**: Soroban's native auth framework correctly validates that the specified address signed the transaction.

### 2.2 Oracle Assumptions

- **Minimum 2 registered oracles required**: The consensus mechanism requires at least 2 price submissions before calculating a consensus price. Single-oracle scenarios are rejected.
- **Price staleness threshold is 300 seconds**: Prices older than 5 minutes are discarded. This is a known trade-off between freshness and availability.
- **10% deviation threshold**: If the spread between min and max submitted prices exceeds 10%, the price is rejected as `UnreliablePrice`. Attackers cannot silently manipulate prices beyond this band.
- **Oracle reputation slashing at 20% deviation**: Oracles deviating more than 20% from consensus are slashed and may be removed if reputation falls below threshold.

### 2.3 Staking Assumptions

- **Minimum stake of 100 XLM (100_000_000 stroops)**: Providers must maintain this stake to submit signals. This is a spam deterrent, not a security guarantee.
- **7-day unstake lock after signal submission**: Prevents providers from submitting signals and immediately withdrawing stake before outcomes are known.
- **Stake is not slashed on-chain in current implementation**: Slashing is tracked via reputation scores; actual token slashing requires a future upgrade with token contract integration.

### 2.4 Access Control Assumptions

- **Admin key security is out of scope**: We assume the deployer secures the admin private key. Compromise of the admin key compromises all admin-gated functions.
- **Multi-sig is optional but recommended for mainnet**: The multi-sig module is implemented but not enforced at deployment. Auditors should verify the multi-sig path is exercised in tests.
- **`require_auth()` is called before all state mutations**: Every public function that mutates state calls `address.require_auth()` before any storage writes.

---

## 3. Known Limitations

### 3.1 Functional Limitations

| # | Limitation | Impact | Mitigation |
|---|---|---|---|
| L-1 | Stake is tracked in contract storage, not via actual token transfers | Provider can claim stake without holding XLM | Requires token contract integration before mainnet |
| L-2 | `is_sell` flag in `execute_trade` is hardcoded to `false` | Stop-loss checks for sells are never triggered | Must be derived from signal action before mainnet |
| L-3 | SDEX orderbook fetch (`fetch_sdex_orderbook`) is `unimplemented!()` | `refresh_from_sdex` will panic if called | Mark as experimental; do not expose on mainnet |
| L-4 | Signal storage uses `Map<u64, Signal>` in instance storage | Instance storage has size limits; large signal counts may hit ledger entry limits | Migrate to persistent storage with pagination |
| L-5 | Bubble sort used for leaderboard and oracle median | O(n²) complexity; DoS risk with large datasets | Enforce hard caps on collection sizes |
| L-6 | Merge conflict markers present in `signal_registry/src/lib.rs` and `oracle/src/lib.rs` | Compilation may fail; undefined behavior in affected branches | Resolve all merge conflicts before audit submission |
| L-7 | Oracle `initialize` does not store admin in one code path | Admin-gated functions may be uncallable | Unify oracle initialization paths |
| L-8 | `calculate_fee_preview` ignores `_env` parameter | Fee calculation is stateless; cannot reflect dynamic fee changes | Acceptable for preview; document clearly |

### 3.2 Security Limitations

| # | Limitation | Impact | Mitigation |
|---|---|---|---|
| S-1 | No rate limiting on `create_signal` beyond minimum stake | Providers with large stakes can spam signals | Implement per-provider signal rate limit (e.g., max 10/hour) |
| S-2 | No commit-reveal for signal submission | Front-running of signal content is possible | Accept as known risk or implement commit-reveal scheme |
| S-3 | Emergency pause only covers `create_signal`; `execute_trade` has no pause | Trades can continue during a pause event | Extend pause check to `AutoTradeContract.execute_trade` |
| S-4 | `transfer_admin` has no time-lock or confirmation step | Admin key compromise leads to immediate admin transfer | Add 2-step admin transfer with acceptance period |
| S-5 | Oracle weights are stored in persistent storage with no expiry | Stale weight assignments persist indefinitely | Add weight TTL or periodic recalibration |

---

## 4. Security Checklist

### 4.1 Reentrancy

| Item | Status | Notes |
|---|---|---|
| All state changes occur before external calls | ✅ PASS | Soroban's execution model is single-threaded; no cross-contract reentrancy within a single invocation |
| No recursive call patterns identified | ✅ PASS | No contract calls back into itself |
| Storage writes complete before event emissions | ✅ PASS | All `signals.set()` / `save_signals_map()` calls precede `events::emit_*()` |

### 4.2 Integer Arithmetic

| Item | Status | Notes |
|---|---|---|
| Overflow checks enabled in release profile | ✅ PASS | `overflow-checks = true` in `Cargo.toml` `[profile.release]` |
| `checked_add` used for signal ID counter | ✅ PASS | `counter.checked_add(1).expect("signal id overflow")` in `next_signal_id` |
| ROI and fee calculations use checked arithmetic | ⚠️ PARTIAL | `performance::calculate_roi` and `fees::calculate_fee_breakdown` — verify internally |
| Oracle price deviation uses integer division | ⚠️ REVIEW | `(max_p - min_p) * 100 / min_p` — potential division by zero if `min_p == 0` |
| Portfolio value calculation uses basis-point division | ⚠️ REVIEW | `position.amount * price / 100` — precision loss for small amounts |

### 4.3 Access Control

| Item | Status | Notes |
|---|---|---|
| `initialize` is one-time only | ✅ PASS | `has_admin` guard prevents re-initialization |
| All admin functions call `require_admin` | ✅ PASS | Verified in `admin.rs` for all setter functions |
| `require_auth()` called on all user-facing mutations | ✅ PASS | `provider.require_auth()`, `user.require_auth()`, `executor.require_auth()` present |
| Multi-sig threshold enforced on signer removal | ✅ PASS | `signers.len() - 1 < threshold` check in `remove_multisig_signer` |
| Oracle registration is admin-gated | ✅ PASS | `register_oracle` calls `require_admin` |
| Authorization expiry checked on every trade | ✅ PASS | `current_time < cfg.expires_at` in `is_authorized` |

### 4.4 Input Validation

| Item | Status | Notes |
|---|---|---|
| Asset pair format validated | ✅ PASS | `validate_asset_pair_common` from `common` crate |
| Signal expiry bounds checked (future, max 30 days) | ✅ PASS | `expiry <= now` and `expiry > now + MAX_EXPIRY_SECONDS` |
| Trade amount must be positive | ✅ PASS | `amount <= 0` check at top of `execute_trade` |
| Oracle price must be positive | ✅ PASS | `price <= 0` returns `InvalidPrice` / `InvalidAsset` |
| Fee rate capped at `MAX_FEE_BPS` (100 bps = 1%) | ✅ PASS | `new_fee_bps > MAX_FEE_BPS` check in `set_trade_fee` |
| Risk parameters capped at 100% | ✅ PASS | `stop_loss > MAX_RISK_PERCENTAGE` check |
| Tag count limited to 10 per signal | ✅ PASS | `signal.tags.len() + tags.len() > 10` check |
| Multi-sig threshold cannot exceed signer count | ✅ PASS | `threshold > signers.len()` check |
| Collaboration contribution percentages validated | ⚠️ REVIEW | Verify sum == 100% in `collaboration.rs` |

### 4.5 Rate Limiting

| Item | Status | Notes |
|---|---|---|
| Daily trade limit per user | ✅ PASS | `check_daily_trade_limit` enforces `config.daily_trade_limit` |
| Signal submission rate limiting | ❌ MISSING | No per-provider rate limit beyond minimum stake |
| Oracle submission rate limiting | ❌ MISSING | No cooldown between oracle price submissions |

### 4.6 Replay Protection

| Item | Status | Notes |
|---|---|---|
| Transaction replay protection | ✅ PASS | Handled by Stellar network sequence numbers |
| Signal ID uniqueness | ✅ PASS | Monotonically incrementing counter with overflow check |
| External signal import deduplication | ✅ PASS | `ExternalIdMappings` storage key prevents duplicate imports |
| Authorization grant idempotency | ✅ PASS | Overwriting existing auth config is safe (latest wins) |

### 4.7 Emergency Controls

| Item | Status | Notes |
|---|---|---|
| Trading pause mechanism | ✅ PASS | `pause_trading` / `unpause_trading` with 48-hour auto-expiry |
| Pause covers signal creation | ✅ PASS | `require_not_paused` called in `create_signal_internal` |
| Pause covers trade execution | ❌ MISSING | `AutoTradeContract.execute_trade` does not check pause state |
| Pause emits events | ✅ PASS | `emit_trading_paused` / `emit_trading_unpaused` |

### 4.8 Secrets and Keys

| Item | Status | Notes |
|---|---|---|
| No hardcoded private keys | ✅ PASS | No secrets in source code |
| No hardcoded contract addresses | ✅ PASS | All addresses passed as parameters |
| Admin address set at initialization | ✅ PASS | Passed to `initialize(admin)` at deploy time |

### 4.9 Event Emissions

| Item | Status | Notes |
|---|---|---|
| Signal creation emits event | ⚠️ REVIEW | Verify `events::emit_signal_created` is called in `create_signal_internal` |
| Trade execution emits event | ✅ PASS | `emit_trade_executed` called in `record_trade_execution` |
| Admin parameter changes emit events | ✅ PASS | `emit_parameter_updated` called in all setters |
| Admin transfer emits event | ✅ PASS | `emit_admin_transferred` called |
| Oracle consensus emits event | ✅ PASS | `emit_consensus_reached` called |
| Oracle slashing emits event | ✅ PASS | `emit_oracle_slashed` called |
| Pause/unpause emits events | ✅ PASS | Dedicated emit functions |

### 4.10 Gas / Compute

| Item | Status | Notes |
|---|---|---|
| Bubble sort on unbounded collections | ❌ RISK | `get_top_providers`, `weighted_median`, `get_leaderboard` — O(n²) |
| Signal filtering iterates all signals | ❌ RISK | `get_signals_filtered` iterates entire signal map |
| Hard limit on leaderboard results (max 50) | ✅ PASS | `limit.min(50)` enforced in `get_leaderboard` |
| Batch cleanup limits enforced | ✅ PASS | `cleanup_expired_signals(limit)` respects batch size |

---

## 5. Test Coverage Summary

See `tests/coverage_report.sh` for automated generation.

### 5.1 Coverage Targets

| Contract | Target Line Coverage | Target Branch Coverage |
|---|---|---|
| `signal_registry` | ≥ 95% | ≥ 90% |
| `auto_trade` | ≥ 95% | ≥ 90% |
| `oracle` | ≥ 95% | ≥ 90% |
| `common` | ≥ 95% | ≥ 90% |

### 5.2 Test Files

| Contract | Test File | Focus Areas |
|---|---|---|
| `signal_registry` | `src/test.rs` | Core signal CRUD, admin, pause, social |
| `signal_registry` | `src/test_analytics.rs` | Analytics, trending assets, global metrics |
| `signal_registry` | `src/test_categories.rs` | Categorization, tag filtering, risk levels |
| `signal_registry` | `src/test_performance.rs` | ROI calculation, provider stats, leaderboard |
| `signal_registry` | `src/test_collaboration.rs` | Multi-author signals, approval flow |
| `signal_registry` | `src/test_import.rs` | CSV/JSON import, deduplication |
| `auto_trade` | `src/test.rs` | Trade execution, risk checks, auth |
| `auto_trade` | `src/risk.rs` (inline) | Risk config, position limits, stop-loss, daily limits |
| `auto_trade` | `src/stake.rs` (inline) | Stake/unstake, lock period, minimum stake |
| `oracle` | `src/test.rs` | Price submission, consensus, TWAP, deviation |
| `oracle` | `src/validation_tests.rs` | Input validation edge cases |

### 5.3 Critical Security Functions — Test Coverage

| Function | Test Coverage | Notes |
|---|---|---|
| `require_admin` | ✅ Tested | Unauthorized caller rejection |
| `require_not_paused` | ✅ Tested | Signal creation blocked when paused |
| `is_authorized` | ✅ Tested | Expiry, amount limit, revocation |
| `validate_trade` | ✅ Tested | Daily limit, position limit, stop-loss |
| `check_stop_loss` | ✅ Tested | Trigger and non-trigger cases |
| `check_daily_trade_limit` | ✅ Tested | At-limit and over-limit cases |
| `stake` / `unstake` | ✅ Tested | Lock period, minimum stake |
| `get_price_with_confidence` | ✅ Tested | Stale price, deviation rejection |
| `calculate_consensus` | ✅ Tested | Weighted median, slashing |
| `enable_multisig` | ✅ Tested | Duplicate signer, threshold validation |

---

## 6. Audit Package Contents

```
StellarSwipe-Contract/
├── stellar-swipe/
│   ├── contracts/
│   │   ├── signal_registry/src/   ← Primary contract source
│   │   ├── auto_trade/src/        ← Trade execution source
│   │   ├── oracle/src/            ← Oracle aggregation source
│   │   └── common/src/            ← Shared utilities
│   └── Cargo.toml                 ← Workspace manifest
├── docs/
│   ├── security_audit.md          ← This document
│   ├── attack_vectors.md          ← Attack vector analysis
│   ├── ORACLE_INTEGRATION.md      ← Oracle integration details
│   └── AUTHORIZATION_IMPLEMENTATION.md
├── tests/
│   └── coverage_report.sh         ← Coverage generation script
└── README.md
```

### 6.1 Build Instructions

```bash
# Install toolchain
rustup target add wasm32-unknown-unknown
cargo install soroban-cli --locked

# Build all contracts
cd stellar-swipe
cargo build --target wasm32-unknown-unknown --release

# Run all tests
cargo test

# Run with coverage (requires cargo-tarpaulin)
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir ../tests/coverage_output
```

### 6.2 Deploy Instructions (Testnet)

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/signal_registry.wasm \
  --network testnet \
  --source <DEPLOYER_SECRET_KEY>

soroban contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source <ADMIN_SECRET_KEY> \
  -- initialize --admin <ADMIN_ADDRESS>
```

---

## 7. Auditor Guidance

### 7.1 Suggested Audit Focus Areas

1. **Oracle manipulation resistance** — Review `get_price_with_confidence`, deviation check, and weighted median in `oracle/src/lib.rs`
2. **Stake griefing** — Review `stake.rs` minimum stake enforcement and signal rate limiting (currently absent)
3. **Admin key centralization** — Review `admin.rs` transfer and multi-sig paths
4. **Arithmetic safety** — Review all division operations for zero-divisor cases, especially in `risk.rs` portfolio calculations
5. **Merge conflict resolution** — `signal_registry/src/lib.rs` and `oracle/src/lib.rs` contain unresolved merge conflict markers that must be resolved before final audit

### 7.2 Out of Scope

- Frontend / off-chain indexer security
- Stellar network-level attacks
- Private key management
- Third-party oracle data source integrity (Band Protocol)

### 7.3 Reviewer Sign-off

| Reviewer | Role | Date | Signature |
|---|---|---|---|
| TBD | Lead Developer | — | — |
| TBD | Security Reviewer | — | — |
