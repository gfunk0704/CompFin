# Claude Answer 7 — Nominal 門檻、Deposit/Basis Swap 的 PV01、DV01 加總權重

**閱讀範圍**：所有歷史討論紀錄（user_question_1–7、claude_answer_1–6、gemini_answer_1–6）

---

## 一、Nominal = 1,000,000 時門檻是否仍有必要

### 直接結論

門檻**仍然必要**，但功能性質從「防止數值崩潰」轉移到「資料完整性驗證」。

### 論證

在 claude_answer_5 中，我提出 PV01 < 1e-12 時應視為資料錯誤並回傳 `CalibrationError`，而非做任何 fallback。這個立場在 nominal = 1,000,000 的情況下**更加確定**，而非被削弱。

| 商品 | Nominal = 1 的 PV01 (×1e-4) | Nominal = 1,000,000 的 PV01 |
|---|---|---|
| O/N Deposit (τ ≈ 1/252) | ≈ 4 × 10⁻⁷ | ≈ 0.4 |
| 1M Deposit | ≈ 8 × 10⁻⁶ | ≈ 8 |
| 1Y IRS（季付） | ≈ 4 × 10⁻⁵ | ≈ 40 |
| 30Y IRS（半年付） | ≈ 10⁻³ | ≈ 1,000 |

Nominal = 1,000,000 時，最短端商品（O/N Deposit）的 Par PV01 也在 0.4 左右，距離 1e-12 有 12 個數量級的差距。

在這個情境下，PV01 < 1e-12 **唯一可能的觸發條件**是：

1. nominal 被設為 0（資料錯誤）
2. 所有折現因子均為零（市場資料錯誤）
3. 程式邏輯 bug 導致 PV01 計算本身崩潰

這三種情況都應該被明確攔截，而非靜默接受。門檻的存在正是讓這類錯誤在校準開始前就被發現，而非在後續迭代中悄悄造成數值問題。

**立場維持**：門檻設為 1e-12，觸發時回傳 `CalibrationError::DegenerateInstrument`，不做任何 fallback。Nominal = 1,000,000 只是讓觸發門檻更顯然是 bug，不影響設計原則。

---

## 二、Deposit 作為 Coupon Bond 的 analytic_pv01

### 程式確認

從 `src/instrument/interestrate/deposit.rs` 的實作看到：
- `capitalization_flow_list[0]`：期初支付本金 `-N`，支付日為第一個計算期的 start_date
- `capitalization_flow_list[1]`：期末回收本金 `+N`，支付日為最後一個 payment_date
- `flow_oberver_list`：每個計算期一個 FlowObserver，負責計算並回傳利息

這個結構可以有**多個計算期**，確實比 QuantLib 的 Deposit 更接近 coupon bond。

### 解析式 analytic_pv01 的計算

固定利率多期 Deposit 的 NPV：

$$NPV = -N \cdot D(T_0) + \sum_{k=1}^{n} N \cdot r \cdot \delta_k \cdot D(T_k) + N \cdot D(T_n)$$

對 par rate 偏導：

$$\frac{\partial NPV}{\partial r^{par}} = N \cdot \sum_{k=1}^{n} \delta_k \cdot D(T_k)$$

這正是**年金因子 × 本金**，與 n = 1 的單期 Deposit 完全一致（退化為 $N \cdot \delta \cdot D(T)$）。

對浮動利率 Deposit（quote_target = spread on floating leg）：

$$\frac{\partial NPV}{\partial \text{spread}} = N \cdot \sum_{k=1}^{n} \delta_k \cdot D(T_k)$$

形式完全相同。

**多期結構不使公式複雜化**——它只是讓加總項變多。Coupon bond 結構的一個數學上的優雅特性：par PV01 永遠是所有付息期間的折現加權天數之和 × 本金，與期數無關。

實作建議（直接以 `flow_oberver_list` 迭代）：

```rust
// Deposit（適用 fixed 或 floating，無論幾個計算期）
fn analytic_pv01(&self, discount_curve: &Arc<dyn DiscountCurve>) -> Result<f64, PricerError> {
    let annuity: f64 = self.flow_oberver_list
        .iter()
        .map(|fo| fo.day_count_fraction() * discount_curve.discount(fo.payment_date()))
        .sum();
    Ok(annuity * self.nominal.abs() * 1e-4)
}
```

**結論：analytic_pv01 的公式在 coupon bond 結構下完全相同，不需要任何特殊處理。**

---

## 三、InterestRateSwap 作為 Basis Swap：r^par 計算是否複雜化

### 直接回答

**analytic_pv01 的計算不會複雜化**；複雜化的是 par rate 本身的計算，但 `analytic_pv01` 不需要 par rate。

### 論證

對標準固定 vs 浮動 IRS，quote_target = 固定利率：

$$\frac{\partial NPV}{\partial r^{fixed}} = -N \cdot A_{fixed} = -N \cdot \sum_j \delta_j^{fixed} \cdot D(T_j^{fixed})$$

對 basis swap（floating vs floating），quote_target = spread on receive leg：

$$NPV = \sum_i (f_i^{receive} + s) \cdot \delta_i^{receive} \cdot D(T_i^{receive}) - \sum_j f_j^{pay} \cdot \delta_j^{pay} \cdot D(T_j^{pay})$$

$$\frac{\partial NPV}{\partial s} = \sum_i \delta_i^{receive} \cdot D(T_i^{receive}) = A_{receive}$$

一樣是年金因子，一樣可以解析計算，一樣是 O(n) 操作。

**但有一個實作問題**：`analytic_pv01` 作為 `Instrument` trait 的方法，需要知道 quote_target 是固定利率、pay-leg spread 還是 receive-leg spread——否則對 basis swap 無法決定計算哪條腿的年金因子。

有兩個解法：

**解法 A**：讓 `analytic_pv01` 接受 `QuoteTarget` 參數：

```rust
pub enum QuoteTarget { FixedRate, SpreadOnPay, SpreadOnReceive }

fn analytic_pv01(
    &self,
    quote_target: QuoteTarget,
    discount_curve: &Arc<dyn DiscountCurve>,
) -> Result<f64, PricerError>;
```

**解法 B**（更乾淨）：由 `InterestRateCurveCalibrationHelper` 在生成時計算並儲存 PV01，而非把計算推遲到 LM 迭代時。`generate_calibration_helper()` 已知 quote_target，可在此時計算年金因子，避免讓 instrument 自己知道校準的 quote 型別。

解法 B 的好處是把「與市場 quote 相關的知識」集中在 calibration layer，不污染 instrument 層。

**結論：basis swap 的 analytic_pv01 不複雜，但需要在 API 層面明確 quote_target 的歸屬。**

---

## 四、攻擊 DV01 (Zero Rate) 加總作為權重的最弱假設

### 首先釐清公式的意圖

Ray 的公式中使用了同一個下標 $i$ 作為外層加總和商品索引，這是 LaTeX typo。根據「各個 pillar 的 DV01 (zero rate) 加總」的文字說明，正確理解應為：

$$\omega_i = \sum_{j=1}^{N} \frac{\partial NPV_i}{\partial r_j}$$

即商品 $i$ 對所有 pillar 零利率變動的敏感度之加總，這等同於**平行移動 DV01（Parallel Shift DV01）**。

### 最弱假設

Parallel DV01 加總方案的最弱、也是技術上最根本錯誤的假設是：

**「Parallel DV01 加總後，不同期限商品 1bp 的利率誤差在 LM 目標函數中的貢獻趨於對等，實現公平的利率空間比較。」**

這個假設是錯的，而且方向恰好相反。

### 論證一：Parallel DV01 = Par PV01 × 存續期

對一個到期日為 $T$ 的 par IRS（連續複利，近似平坦曲線），對插值基礎為 zero rate 的曲線：

$$\sum_j \frac{\partial D(t_k)}{\partial r_j} = -t_k \cdot D(t_k)$$

（這來自任何分拆合一（partition of unity）的插值方案：$\sum_j w_j(t_k) = 1$，故 $\sum_j \partial r(t_k)/\partial r_j = 1$，代入 $\partial D(t_k)/\partial r(t_k) = -t_k \cdot D(t_k)$ 即得。）

因此：

$$\text{Parallel DV01}_i = \sum_j \frac{\partial NPV_i}{\partial r_j} = -N \cdot \sum_k CF_k \cdot t_k \cdot D(t_k) \approx \text{Par PV01}_i \times t_{eff,i}$$

其中 $t_{eff}$ 是修正存續期（Modified Duration），對 par IRS 近似等於 $T/2$（平坦曲線）。

**量化影響：**

| 商品 | Par PV01（N=1M, bp） | Parallel DV01 | 比值 ≈ $t_{eff}$ |
|---|---|---|---|
| 1Y IRS（季付） | 95 | 47 | 0.5 |
| 5Y IRS | 450 | 1,100 | 2.4 |
| 10Y IRS | 800 | 4,000 | 5 |
| 30Y IRS | 1,800 | 27,000 | 15 |

### 論證二：Parallel DV01 縮放製造存續期偏差，而非消除它

使用 Parallel DV01 作為縮放因子，殘差 $f_i = NPV_i / \text{DV01}_i$ 的量綱是「利率偏差 ÷ 存續期」：

$$f_i \approx \frac{(r_i^{model} - r_i^{market}) \cdot \text{Par PV01}_i}{\text{Par PV01}_i \times t_{eff,i}} = \frac{r_i^{model} - r_i^{market}}{t_{eff,i}}$$

對兩個商品各有 1bp 的利率誤差：
- 1Y IRS：$f_{1Y} \approx 1\text{bp} / 0.5 = 2\text{bp}$
- 30Y IRS：$f_{30Y} \approx 1\text{bp} / 15 \approx 0.067\text{bp}$

**同樣是 1bp 的利率錯誤，Parallel DV01 縮放後短端殘差比長端大 30 倍。** LM 演算法在最小化 $\sum f_i^2$ 時，會把 30 倍更多的「關注度」放在 1Y IRS 上，而系統性忽視 30Y IRS 的校準精度。

Par PV01 縮放後，同樣 1bp 誤差的兩個商品殘差相等，才是真正的「利率空間轉換」。

### 論證三：浮動腳的代數抵消問題

對帶有浮動腳的 IRS 或 basis swap，NPV 對 zero rate $r_j$ 的偏導包含兩個方向的貢獻：

- 固定腳付款：$\partial NPV / \partial r_j < 0$（折現因子 → DPV 下降）
- 浮動腳收款的 forward rate 成份：$\partial NPV / \partial r_j > 0$（forward rate 上升）

在接近 at-money 的 par IRS 中，這兩個貢獻幾乎抵消——實際上，`Σ_j ∂NPV/∂r_j` 對 par IRS 遠小於 Par PV01（因為浮動腳的收益幾乎精確補償折現因子下降）。

**極端情況**：對一個短期 floating vs floating basis swap（兩腿參照相近 tenor 的不同曲線），兩腿的 Parallel DV01 可能幾乎完全抵消，導致 $\omega_i = \sum_j \partial NPV_i / \partial r_j \approx 0$。這等效於 Gemini 在 answer_5 中批評 Claude 的 ε fallback 問題——用一個因代數抵消而趨近零的量來正規化，比 fallback to 1.0 更差，因為它不僅失去縮放意義，還引入隨機符號。

**這與 Gemini 用於批評我的 ε 門檻的論點（「目標函數 landscape 出現斷層」）完全一致，但 Parallel DV01 縮放在結構上更脆弱：它的數值退化是由商品本身的現金流結構決定的，無法用 clamping 修補。**

### 立場聲明

**不接受 Parallel DV01（加總）作為權重。**

此方案比 Par PV01 差（引入存續期偏差），且在浮動腳存在時可能因代數抵消而比 key-rate DV01 更不穩定。

**維持 claude_answer_5/6 的立場：Par PV01（$N \cdot \sum_k \delta_k \cdot D(T_k) \times 1\text{bp}$）是最合適的縮放因子。**

---

## 五、立場匯整

| 議題 | Ray 的問題 | 我的立場 |
|---|---|---|
| 門檻必要性（nominal = 1M） | 是否可取消？ | 仍必要，作資料驗證；nominal = 1M 只是讓觸發更顯然是 bug |
| Deposit coupon bond 結構 | 是否影響 analytic_pv01？ | 不影響；公式 = $N \cdot \sum_k \delta_k \cdot D(T_k)$ 對所有期數成立 |
| Basis swap r^par | PV01 計算是否複雜？ | r^par 複雜，但 analytic_pv01 只需年金因子；quote_target 歸屬建議放在 helper 層 |
| DV01 加總權重 | 作為替代方案是否合適？ | **反對**：引入存續期偏差，系統性壓制長端殘差；浮動腳存在時可因代數抵消崩潰 |
