# Claude 執行摘要 (4) - CompoundingRateIndex 條件式快取包覆

## 修改檔案

`src/interestrate/index/interestrateindexmanager.rs`，`build_compounding_rate_index` 函式結尾。

## 修改內容

**改動前：**
```rust
let raw = Arc::new(CompoundingRateIndex::with_options(...));
Ok(Arc::new(MultiThreadedCachedIndex::new_threadsafe(raw)))
```

**改動後：**
```rust
let raw = Arc::new(CompoundingRateIndex::with_options(...));
// 若 Index 滿足 AF 條件，系統將不會對其進行快取包覆。若手動關閉 AF 模式，請注意效能損耗。
if !raw.arbitrage_free_applicable() {
    return Ok(Arc::new(MultiThreadedCachedIndex::new_threadsafe(raw)));
}
Ok(raw)
```

## 行為對比

| Index 類型 | 改動前 | 改動後 |
|---|---|---|
| AF-applicable（無 lockout/lookback/Arrear） | 包覆（多餘 overhead ~50 ns/call） | **不包覆** |
| non-AF（有 lockout/lookback/Arrear） | 包覆 | **維持包覆** |

## 設計依據

依據 `claude_answer_3.md` 的分析：
- AF-applicable index 走 `arbitrage_free_factor()`，成本 ~20 ns，低於 cache overhead（~50 ns）
- Bump-and-reprice（DV01）每次換 curve pointer → 永遠 cache miss，包覆無益
- non-AF index 走 `standard_forward_factor()`，成本 ~3–5 μs，cache 有效節省 ~98%

## 驗證

`cargo check` 通過，無新增 warnings 或 errors。
