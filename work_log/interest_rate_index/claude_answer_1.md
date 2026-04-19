# Claude 回覆 - InterestRateIndex Cache 討論

## Q1：Rust 版本中 cache 是否同樣能顯著提升速度？

**結論：對 `CompoundingRateIndex` 有顯著效果；對 `TermRateIndex` 幾乎沒有效益，加上 cache 反而可能更慢。**

原因在於兩類 index 的計算成本截然不同：

| Index 類型 | `projected_rate_for_period` 計算成本（Rust） | Cache 是否值得 |
|---|---|---|
| `TermRateIndex` | ~10–20 ns（2 次 DF lookup + 1 次 `implied_rate`） | 不值得（RwLock 開銷 ~20–50 ns，大於計算本身） |
| `CompoundingRateIndex`（3M period） | ~3–5 μs（~60 個業務日 × DF lookup + fold） | 值得（lock 開銷 < 1% of 計算） |

Python 版本的 cache 收益非常顯著，是因為 Python 本身的 object dispatch 和 loop 都非常慢（~50–100 μs），任何操作相較 cache 的字典查詢都是高成本。Rust 裡這個前提只對 `CompoundingRateIndex` 成立，`TermRateIndex` 的計算成本太低，lock overhead 會 dominate。

### 特別注意：Python 程式碼中有兩個 bug

```python
# Bug 1：應是清空 cache dict，而不是把 UUID 設成空 dict
self.__cachedCurveUUID = {}       # ← 應為 self.__cache = {}

# Bug 2：每次 miss 都把整個 cache 換掉，而非只插入當次結果
self.__cache = self._calculate(d, forwardCurve)  # ← 應為 self.__cache[d] = ...
```

---

## Q2：是否應該讓所有 index 預設都被 `CachedInterestRateIndex` 包覆？

**結論：不應全部預設；正確做法是在 manager loader 層，只對 `CompoundingRateIndex` 自動包覆。**

### Rust 的關鍵架構限制

Python 中可以在 base class 的 `__init__` 裡放 `self.__cache = {}`，因為 Python 物件預設不需要 thread-safe。Rust 中要在 `Arc<dyn InterestRateIndex + Send + Sync>` 裡支援內部 cache，必須使用 `RwLock`，否則：

- `RefCellBackend` → `!Sync`，無法放入 `Arc<dyn ... + Send + Sync>`，frozen manager 無法儲存
- `RwLockBackend` → 每次讀取都有 lock 成本

所以 Python 那樣「base class 預設 cache」的模式，在 Rust 的 frozen manager 架構下必須改為 `RwLockBackend` decorator，而這個成本只對高計算成本的 index 才合理。

### 建議做法：在 loader 層針對 `CompoundingRateIndex` 自動包覆

修改 `interestrateindexmanager.rs` 的 `build_compounding_rate_index`，在回傳前包一層 `MultiThreadedCachedIndex`：

```rust
// interestrateindexmanager.rs — build_compounding_rate_index 最後兩行
// 原本：
Ok(Arc::new(CompoundingRateIndex::with_options(...)))

// 改為：
use crate::interestrate::index::cachedinterestrateindex::CachedInterestRateIndex;
let raw = Arc::new(CompoundingRateIndex::with_options(...));
Ok(Arc::new(CachedInterestRateIndex::new_threadsafe(raw)))
```

這個修改只在一處（loader），不影響 `InterestRateIndex` trait 本身、`CachedInterestRateIndex` 設計、或任何使用端。

`TermRateIndex` 維持不包覆。

### 對 bootstrapping 的效益

`IterativeBootstrapper` 在每個 pillar 的求解迭代中，會對同一個 `(curve_ptr, period.start_date())` 組合反覆呼叫 `projected_rate_for_period`。以 SOFR compounding index、90-day period、100 次 root solver 迭代為例：

- 無 cache：100 × ~4 μs = 400 μs per pillar
- 有 cache：第 1 次計算 ~4 μs；之後 99 次各 ~50 ns = 第 1 次後只需 ~5 μs
- 節省比例：約 98%

這與 `FreezableInstrument` 的前綴凍結是互補的優化：前者快取 index rate 計算，後者快取已解 pillar 的 NPV。

---

## 結論摘要

| 問題 | 回答 |
|---|---|
| Rust 版本 cache 有效果嗎？ | 對 `CompoundingRateIndex` 有顯著效果（~98% 節省 in bootstrapping）；對 `TermRateIndex` 沒有效益 |
| 是否應讓所有 index 預設使用 cache？ | 不應全部預設；在 `build_compounding_rate_index` loader 自動包覆 `MultiThreadedCachedIndex` 即可 |
| 對應到 Python 設計的轉換 | Python 的 base class cache → Rust 的 loader 層 decorator（因 Send+Sync 限制） |
