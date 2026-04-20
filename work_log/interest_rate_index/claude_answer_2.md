# Claude 回覆 - CompoundingRateIndex 與 arbitrage_free_factor 等價性確認

## 核心結論

**不需要進一步的架構修改或確認動作。** 現有設計在 vanilla 定價範圍內已完整、一致，以下做詳細推理說明。

---

## 數學等價性的確認

當三個條件同時成立（`lookback_days == 0`、`fixing_convention == Advance`、`lockout_days == 0`），`use_arbitrage_free` 預設自動為 `true`。此時：

```
CompoundingRateIndex.projected_rate_for_period:
  compound_factor = D(start) / D(end)          // arbitrage_free_factor()
  → result_compounding.implied_rate(factor, τ)

TermRateIndex.projected_rate_for_period:
  fv = D(start) / D(end)
  → compounding.implied_rate(fv, τ)
```

兩者在相同 compounding convention 下，`projected_rate_for_period` 輸出**數學上完全相同**。這個等價性是成立的，不需要額外驗證。

---

## 設計層面：兩個 Index 仍有實質區別

即使 `projected_rate_for_period` 等價，兩個 Index 在 **`fixing_rate_for_period`** 的行為仍然不同，且這個區別是**刻意且正確**的：

| | `TermRateIndex` | `CompoundingRateIndex` |
|---|---|---|
| `projected_rate_for_period` (use_AF=true) | D(start)/D(end) | D(start)/D(end)（等價） |
| `fixing_rate_for_period`（過去/混合期） | 查 `past_fixings[fixing_date]`（單一 term rate） | `compute_compound_factor_mixed()`（逐日過去 fixing 乘積） |
| 語意 | 已知的 3M term rate（如 LIBOR 風格） | SOFR 每日複利累計值 |

`fixing_rate_for_period` 的路徑 `use_arbitrage_free` 旗標**完全不影響**，因此兩個 Index 仍然是語意上不可互換的型別。Bootstrapping 只使用 `projected_rate_for_period`（全部 future），所以在 vanilla 校準場景中，等價性成立且沒有語意衝突。

---

## 不使用 arbitrage_free_factor 的情境（DV01）

用戶已正確識別：**不使用** `arbitrage_free_factor` 的主要場景是風險敏感度計算（DV01 / Greeks）。

原因如下：

```
arbitrage_free_factor 路徑：
  rate 僅對 D(start) 和 D(end) 有敏感度
  → 曲線中間節點的 bump 不會產生任何 P&L 反應
  → DV01 向量嚴重低估（只有頭尾 pillar 有值）

standard_forward_factor 路徑：
  每個業務日 d_i 對應 D(d_i)/D(d_{i+1}) 的比值
  → 每個 overnight pillar 都有對應敏感度
  → DV01 向量正確反映各節點的貢獻
```

因此，DV01 計算時需呼叫 `index.set_use_arbitrage_free(false)` 切回逐日計算。現行 API 已支援此切換，接線邏輯留給未來 Greeks 模組實作，**現在不需要任何預先修改**。

---

## Cache 包覆的效益重新評估（與 Action 1 的關聯）

Action 1 為所有 `CompoundingRateIndex` 在 loader 層自動包覆 `MultiThreadedCachedIndex`。這裡需要補充一個細節：

| 路徑 | 計算成本 | Cache 效益 |
|---|---|---|
| `use_arbitrage_free = true` | ~2 DF lookups（~20 ns） | 極低：cache 的 RwLock overhead ≈ 計算本身 |
| `use_arbitrage_free = false`（DV01 或有 lockout/lookback 時） | ~60 DF lookups（~3–5 μs） | 顯著（~98% bootstrapping 節省） |

這意味著：對於 vanilla 場景（`use_arbitrage_free = true`），`MultiThreadedCachedIndex` 在 projected_rate 路徑的收益接近零。但**不建議移除** cache 包覆，原因是：

1. 切換回 `use_arbitrage_free = false`（如 DV01）時，cache 效益立即恢復
2. `fixing_rate_for_period`（mixed 期間的 `compute_compound_factor_mixed`）永遠是 ~3 μs，cache 對這條路徑有收益
3. loader 層只改一處，維護成本極低

---

## 結論摘要

| 問題 | 結論 |
|---|---|
| vanilla 定價範圍是否需要進一步確認？ | 不需要，`arbitrage_free_factor` 已正確自動啟用，等價性數學上成立 |
| `CompoundingRateIndex` 是否可被 `TermRateIndex` 完全取代？ | 不行，`fixing_rate_for_period` 語意不同（逐日乘積 vs. 單一 term rate 查詢） |
| DV01 計算需要什麼？ | `set_use_arbitrage_free(false)`，留待 Greeks 模組接線，現在不需預先修改 |
| Action 1 的 cache 包覆仍然合理嗎？ | 合理；AF 路徑收益低但無害，non-AF 路徑與 mixed 期間仍顯著受益 |
