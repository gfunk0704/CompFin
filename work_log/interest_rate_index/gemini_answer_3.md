# Gemini 回覆 (3) - CompoundingRateIndex 選擇性快取討論

## Q：是否應只針對「無法使用 `arbitrage_free_factor`」的 Index 進行快取包覆？

**結論：建議維持「一視同仁」全部包覆的策略，不建議進行選擇性包覆。**

雖然您的觀察非常敏銳——在 `arbitrage_free_factor` 下，`CompoundingRateIndex` 的計算開銷確實降到了與 `TermRateIndex` 同等級，使得快取的邊際收益變低甚至為負（Lock overhead > 計算開銷）。但基於以下幾個關鍵理由，全部包覆仍是更優的工程選擇：

### 1. `use_arbitrage_free` 是可變狀態 (Mutable State)

這是最重要的原因。在 `CompoundingRateIndex` 的實作中：
- `arbitrage_free_applicable` 是**靜態屬性**（由 lookback/lockout 決定）。
- `use_arbitrage_free` 是**動態狀態**（可透過 `set_use_arbitrage_free` 切換）。

即使一個 Index 在數學上滿足 `arbitrage_free_applicable`，當進入 **DV01 計算模式** 時，我們會將其切換為 `use_arbitrage_free = false`。
- **如果我們採用選擇性包覆**：這個 Index 因為滿足 AF 條件而沒有被 cache 包覆，但在 DV01 模式下它會執行極慢的逐日累計（~3-5 μs）。這會導致風險計算的速度大幅下降。
- **如果我們一視同仁包覆**：在 DV01 模式下，即便關閉了 AF shortcut，快取機制仍能發揮作用（節省 ~98% 的重複計算），確保風險分析的效能。

### 2. `fixing_rate_for_period` 的一致性

如 Claude 所述，無論 `use_arbitrage_free` 是否開啟，只要涉及過去或混合期間的 `fixing_rate_for_period`，系統永遠會執行 `compute_compound_factor_mixed`（逐日邏輯）。
- 只要商品跨越了 Reset Date（例如一個正在進行中的 Coupon 期間），計算成本就會上升。
- 全部包覆可以確保這類「混合期」的評價始終受到快取保護。

### 3. 效能損耗在 Vanilla 路徑中微不足道

在 `use_arbitrage_free = true` 的路徑下：
- 計算成本：~20 ns
- 快取鎖成本：~30-50 ns
- 總計：~70 ns

雖然這比純計算慢了幾倍，但在宏觀的評價流程中（例如一個 Portfolio 定價需要幾毫秒），這幾十納秒的差異完全可以忽略。我們寧願在高速路徑損失一點點效能，也要確保在低速路徑（DV01 / 混合期）有絕對的保護。

### 4. 實作的簡潔性

在 `InterestRateIndexLoader` 中，判斷「是否應包覆快取」的邏輯越簡單，系統就越穩定。
- **簡單邏輯**：如果是 `CompoundingRateIndex` 類別，就包覆。
- **複雜邏輯**：如果是 `CompoundingRateIndex` 且其 `arbitrage_free_applicable` 為 false，才包覆。
後者引入了對內部參數的依賴，增加了程式碼的耦合度，且如前所述，並未考慮到動態切換的需求。

---

## 總結建議

| 方案 | 優點 | 缺點 | 結論 |
|---|---|---|---|
| **選擇性包覆** | 讓 AF-capable 的 Index 在 vanilla 定價時達到極速（20ns）。 | **DV01 計算會變得極慢**；混合期評價無加速。 | ❌ 不建議 |
| **一視同仁包覆** | **確保 DV01 與混合期評價的效能**；邏輯簡單。 | Vanilla 定價時多出幾十 ns 的鎖開銷。 | ✅ **推薦方案** |

我們應該把 `CachedInterestRateIndex` 視為 `CompoundingRateIndex` 的「標準配備」，用來保證其在任何模式下都有穩定的效能表現。
