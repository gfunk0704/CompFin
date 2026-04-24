# Gemini Answer 4 — 實作方案確認與技術細節深度探討

**閱讀範圍**：`user_question_4.md` 及所有歷史討論紀錄。

---

## 一、Least Square 權重與 PillarAlignment：全面支持

### 1.1 Scaling 方案：$f_i(x) = \frac{NPV_i(x)}{Market\_PV01_i}$
我完全支持此方案。這在數值工程上是一個極其優雅且低成本的解法：
- **消除量綱差異**：將所有殘差拉回到「利率空間（Rate Space）」，量級約為 Basis Points，這能極大改善 Jacobian 矩陣的條件數。
- **計算效率**：$Market\_PV01$ 作為校準開始前的常數，不會增加 LM 迭代內部的負擔。
- **穩健性**：即便 initial guess 略有偏差，其 PV01 的量級通常也是正確的，足以達到 Scaling 的目的。

### 1.2 PillarAlignment 實作
支持在 `Instrument` trait 中新增 `maturity_date()`。
- **業務合理性**：`maturity_date` 代表合約名義上的到期日（如 10Y Swap 的最後一天），而 `max_date` 代表最後一個現金流支付日（支付日可能受假日調整後移）。
- **顯式優於隱式**：透過 `PillarAlignment` 讓不同的 Calibrator 顯式聲明其對齊邏輯，能避免模型風險。

---

## 二、PrecomputedDiscountCurve 實作：架構優化與風險提示

針對 User 提出的 Wrapper 與 `requiredDates` 方案，我認為這比 Claude 之前的建議更具擴展性。

### 2.1 攻擊 Claude 的假設：Precomputed 僅對 OIS 有益？
Claude 在 answer_3 中認為 Precomputed 的主要受益對象是 `CompoundingRateIndex`。
**我認為這個假設低估了全域優化（Least Square）的計算規模。**

在 `LeastSquareCalibrator` 中，如果我們有 50 個 Pillar，計算一次 Jacobian 需要評估 51 次投資組合（Finite Difference）。
- 假設有 50 個商品，每個商品平均有 10 個日期（Fixing + Payment）。
- 每次 LM 迭代的 `discount()` 呼叫次數 = $51 \times 50 \times 10 = 25,500$ 次。
- 雖然單次多項式插值很快，但 25,000 次累積的計算成本，加上 PiecewisePolynomial 內部的 Binary Search 開銷，絕對值得透過一次性的 `PrecomputedDiscountCurve::Dense` (Vec 索引 ~7ns) 來優化。

**結論：PrecomputedDiscountCurve 應該作為 `LeastSquareCalibrator` 的「標準預設」，而非僅針對 OIS。**

### 2.2 `requiredDates` 的精準度風險
User 提出的 `requiredDates` 方案在封裝性上優於 Claude 的「手動遍歷」。但有一個核心風險：**「實現的一致性」**。

如果 `requiredDates` 漏掉了一個日期，而 pricer 在計算時卻查詢了該日期，`PrecomputedDiscountCurve` 將面臨 Panic 或返回錯誤結果。
**特別針對 `CompoundingRateIndex`：**
`requiredDates` 的實作必須精確複製 `accrual_to_fixing` 的邏輯，包含 `lookback_days` 與 `lockout_days` 產生的所有 `fixing_date`。
```rust
// 偽代碼：CompoundingRateIndex 的 requiredDates 必須包含：
for d in business_days_in_period {
    dates.insert(accrual_to_fixing(d)); 
    dates.insert(accrual_to_fixing(next_d)); // standard_forward_factor 需要這兩個
}
```

### 2.3 對 Wrapper 實作的修正建議
User 提出的 `PrecomputedDiscountCurveWrapper` 中：
```rust
fn to_zero_rate_curve(&self) -> Arc<dyn ZeroRateCurve> {
    self.base_rate_curve.to_zero_rate_curve()
}
```
這存在一個**效能陷阱**：如果某個 Index 內部呼叫的是 `to_zero_rate_curve()`（例如為了算 Forward Rate），它會繞過 Precomputed 緩存，直接回到昂貴的底層曲線。
**建議**：Wrapper 應該盡可能「全面 Precomputed」，或確保系統內部路徑（如 `InterestRateIndex`）統一走向 `to_discount_curve()`。

---

## 三、最終行動建議

1.  **實作 $Market\_PV01$ Scaling**：在 `InterestRateCurveCalibrationHelper` 中增加一個 `const_pv01` 欄位。
2.  **Instrument 擴充**：在 `Instrument` 新增 `maturity_date()`，並在 `Deposit` / `IRS` 實作。
3.  **RequiredDates 落地**：
    - `SimpleInstrument` 增加 `required_dates(&self, horizon: NaiveDate) -> HashSet<NaiveDate>`。
    - `InterestRateIndex` 同步增加該方法。
    - **重點開發：** `CompoundingRateIndex` 的 `required_dates` 必須經過單元測試，確保與 `standard_forward_factor` 的查詢完全一致。
4.  **Calibrator 優化**：
    - 在 `LeastSquareCalibrator::calibrate()` 開始前，一次性收集所有商品的 `required_dates`。
    - 在 `object_function` (LM 迭代) 內部，利用這些日期建構 `PrecomputedDiscountCurve`。
