# Gemini 回覆 (2) - CompoundingRateIndex Arbitrage-Free Shortcut 討論

## Q：在 `arbitrage_free_applicable` 時，是否應預設使用 `arbitrage_free_factor`？

**結論：是的，對於 Vanilla 商品的評價（MtM）與曲線校準（Bootstrapping），預設使用 shortcut 是業界標準做法。**

目前的 Rust 實作中，`CompoundingRateIndex` 已具備自動偵測與切換機制：
1. `arbitrage_free_applicable` 函數嚴格檢查了 `lookback=0`, `lockout=0` 與 `Advance` 慣例。
2. `use_arbitrage_free` 在建構時預設為 `af_applicable`。

### 1. 為什麼「能用但不使用」的情況少見？

在評價（Pricing）情境下，`D(start)/D(end)` 是基於「無套利」假設的數學恆等式。如果曲線（Curve）本身就是透過這類商品校準出來的，那麼使用 shortcut 能保證數值上的精確對齊（Consistency），且效能提升 100 倍以上。

### 2. 為什麼 DV01 計算時會考慮禁用 Shortcut？

這是最關鍵的技術細節。當我們計算 **Risk / Sensitivities (DV01)**，特別是 **Bucket 敏感度**時：

- **使用 Shortcut (`D(start)/D(end)`)**：
  - 只有當 `start` 或 `end` 落在受影響的插值區間時，才會產生敏感度。
  - 如果中間有一個 Pillar 被微調（Bump），而該 Pillar 不影響 `start` 與 `end` 的折現值（例如使用 Piecewise Polynomial 且 Pillar 在期間中間），則 Shortcut 算出來的 DV01 會是 **0**。這在數學上是正確的（對於該模型而言），但在風險管理上可能遺漏了中間利率變動的影響。

- **不使用 Shortcut（逐日計算 `∏(1 + r_i × δ_i)`）**：
  - 每一天的 `r_i` 都是透過 `D(f_i)/D(f_{i+1})` 計算。
  - 即使受影響的 Pillar 在期間中間，它也會改變受影響天數的 `r_i`，進而改變最終的複合利率。
  - 這能確保 **所有** 相關 Pillar 的敏感度都能被正確捕捉，提供更細緻的風險圖像。

### 3. 對於 `FreezableInstrument` 的影響

在您先前的開發中提到的 `FreezableInstrument` 優化：
- 如果使用 **Shortcut**：計算成本極低，`FreezableInstrument` 的效益不顯著。
- 如果使用 **逐日計算**：計算成本高，`FreezableInstrument` 透過凍結前綴 NPV，能顯著加速校準過程。

這兩者其實是互補的：
- 在 **快速定價/校準** 時：開啟 Shortcut。
- 在 **風險分析/高精度驗證** 時：關閉 Shortcut 並搭配 `FreezableInstrument` 優化。

---

## 建議行動

1. **維持現狀**：目前實作允許透過 `set_use_arbitrage_free(bool)` 動態切換，這已經完美滿足了上述需求。
2. **在 Pricer 層控制**：未來實作 `SimpleInstrumentPricer` 或風險引擎時，可以根據計算目的（MtM vs Risk）來決定是否暫時關閉 Index 的 shortcut。
3. **無需進一步確認**：邏輯已經足夠清晰且符合市場慣例。

---

## 總結

| 場景 | 建議設定 | 原因 |
|---|---|---|
| **MtM Pricing** | `use_arbitrage_free = true` | 追求速度與數值一致性。 |
| **Bootstrapping** | `use_arbitrage_free = true` | 大幅減少迭代中的折現查詢次數。 |
| **Risk / DV01** | `use_arbitrage_free = false` | 確保中間 Pillar 的敏感度不被遺漏。 |
| **Exotic 商品** | 視條款而定 | 只要有 Lockout/Lookback，系統會自動強制轉為 `false`。 |
