# Claude 回覆 - CompoundingRateIndex Cache 包覆策略精化

## 核心結論

**不需要一視同仁。應改為：只對 `arbitrage_free_applicable = false` 的 index 包覆 `MultiThreadedCachedIndex`。**

---

## 計算成本重新分類

| 條件 | `projected_rate_for_period` 路徑 | 單次成本 |
|---|---|---|
| `arbitrage_free_applicable = true` | `arbitrage_free_factor()`：2 次 DF lookup | ~10–20 ns |
| `arbitrage_free_applicable = false` | `standard_forward_factor()`：~60 次 DF lookup + fold | ~3–5 μs |

Cache 本身的開銷：RwLock read + HashMap lookup ≈ **20–50 ns**

---

## 逐情境分析

### 情境 1：`arbitrage_free_applicable = true`（無 lockout / lookback / Arrear）

**Vanilla 定價（`use_arbitrage_free = true`，預設）：**
- 計算成本 ~20 ns，小於或等於 cache overhead
- 即使在 portfolio pricing（多個 swap 共用同一 index），cache 查詢本身也比計算貴
- **結論：cache 無益，甚至有負效益**

**DV01（`set_use_arbitrage_free(false)`）：**
- 計算切換到 `standard_forward_factor()`，成本升至 ~3–5 μs
- 但 DV01 用 bump-and-reprice：每個 bumped curve 是新的 `Arc` 指標 → cache key 不同 → **永遠 cache miss**
- **結論：cache 仍無益**

### 情境 2：`arbitrage_free_applicable = false`（有 lockout 或 lookback 或 Arrear）

- `projected_rate_for_period` 恆走 `standard_forward_factor()` 路徑（~3–5 μs）
- Bootstrapping 中，前綴凍結（`FreezableInstrument`）讓 `frozen_curve` 的指標固定 → 多個 cash flow 對同一 period 命中 cache
- Portfolio pricing 中，多個商品共用同一 period → 每個 period 只算一次
- **結論：cache 顯著有益（~98% 節省）**

---

## 建議修改：條件式包覆

`build_compounding_rate_index` 最後修改如下：

```rust
let raw = Arc::new(CompoundingRateIndex::with_options(
    p.reference_curve_name, p.start_lag, p.adjuster, tenor,
    calendar, fixing_calendar, day_counter, p.daily_past_fixings, p.result_compounding,
    p.lookback_days, p.lockout_days, fixing_conv, missing_fix,
));

// 只對無法使用 arbitrage_free 路徑的 index 包覆 cache
if !raw.arbitrage_free_applicable() {
    return Ok(Arc::new(MultiThreadedCachedIndex::new_threadsafe(raw)));
}
Ok(raw)
```

**改動前後的行為對比：**

| Index 類型 | 改動前 | 改動後 |
|---|---|---|
| AF-applicable（無 lockout/lookback） | 包覆（多餘 overhead） | 不包覆 |
| non-AF（有 lockout/lookback/Arrear） | 包覆（有效） | 包覆（維持） |

---

## 與 Action 1 的關係

Action 1 的包覆決策是在「尚未引入 `arbitrage_free_factor` 計算成本差異」的分析下做出的，當時合理。現在 user_question_2.md 的討論確立了 AF 路徑成本接近 `TermRateIndex`，原本的假設「`CompoundingRateIndex` 恆為高成本」不再成立，因此精化策略是對的。

---

## 結論摘要

| 問題 | 回答 |
|---|---|
| 是否一視同仁包覆所有 `CompoundingRateIndex`？ | 否，應依 `arbitrage_free_applicable()` 條件分流 |
| AF-applicable index 需要 cache 嗎？ | 不需要：AF 路徑成本 ~20 ns，低於 cache overhead |
| DV01 時 AF-applicable index 需要 cache 嗎？ | 不需要：bump-and-reprice 每次換 curve pointer，永遠 cache miss |
| non-AF index（lockout/lookback）仍然需要 cache 嗎？ | 需要：`standard_forward_factor` 恆為 ~3–5 μs，cache 有效 |
