# Claude 實作摘要 - CompoundingRateIndex 自動 Cache 包覆

## 修改內容

**檔案：** `src/interestrate/index/interestrateindexmanager.rs`

**修改一：** 新增 import

```rust
use crate::interestrate::index::cachedinterestrateindex::MultiThreadedCachedIndex;
```

**修改二：** `build_compounding_rate_index` 回傳前包覆一層 `MultiThreadedCachedIndex`

```rust
// 原本
Ok(Arc::new(CompoundingRateIndex::with_options(...)))

// 修改後
let raw = Arc::new(CompoundingRateIndex::with_options(...));
Ok(Arc::new(MultiThreadedCachedIndex::new_threadsafe(raw)))
```

## 設計依據

- 根據 claude_answer_1.md 與 gemini_answer_1.md 的共識：`CompoundingRateIndex` 每次計算約 3–5 μs（60 個業務日 × DF lookup），cache 收益顯著；`TermRateIndex` 計算成本 ~10–30 ns，低於 RwLock 開銷，不包覆。
- 修改只在 loader 一處，不影響 `InterestRateIndex` trait、`CachedInterestRateIndex` 設計、或任何使用端。
- `cargo check` 通過，無新增警告。
