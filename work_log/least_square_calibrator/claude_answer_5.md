# Claude Answer 5 — Market_PV01 定義、ε 門檻、ShiftZeroRateCurve 設計

**閱讀範圍**：所有歷史討論紀錄（user_question_1~5、claude_answer_1~4、gemini_answer_1~4）

---

## 一、Market_PV01_i 的定義：攻擊「單一 pillar shift」

### Ray 的理解

Ray 問：「Market_PV01_i 應該是第 i 個點的 zero rate 平移 1bp 對 NPV_i 的影響（而非整個 rate curve 平移）？」

**這個理解是錯的。** 而且我認為這是本次討論中最重要的澄清點。

### 攻擊「單一 pillar shift」假設的核心論證

Ray 提議的 `ShiftZeroRateCurve` 計算的是**key rate DV01（鍵利率 DV01）**，即：

$$Market\_PV01_i = \frac{NPV_i(\text{curve with pillar } k \text{ shifted } +1bp) - NPV_i(\text{curve})}{\text{1bp}}$$

其中 pillar $k$ 是與商品 $i$ 對應的那個節點。這個量的問題在於：

**問題一：它捕捉的不是「par rate 偏差的成本」。**

$f_i = NPV_i / Market\_PV01_i$ 的設計意圖是讓 $f_i \approx (model\_rate_i - market\_rate_i)$，也就是把 NPV 殘差轉換為「利率偏差（basis points）」。

要讓這個近似成立，我們需要：

$$NPV_i(x) \approx Market\_PV01_i \times (r^{model}_i - r^{market}_i)$$

這裡的 $Market\_PV01_i$ 是 $\frac{\partial NPV_i}{\partial r^{par}_i}$，即對**商品自身的 par rate** 求偏導——也就是**年金因子（annuity factor）**，不是任何形式的 curve shift。

對 IRS：$\frac{\partial NPV_{IRS}}{\partial r^{par}} = -N \cdot \sum_i \delta_i \cdot D(t_i)$（年金因子）

對 Deposit：$\frac{\partial NPV_{Deposit}}{\partial r^{par}} = -N \cdot \delta \cdot D(T)$

這兩個量**不需要任何數值差分**，可以直接從初始 curve 的折現因子計算出來。

**問題二：single-pillar key rate DV01 低估了 par PV01。**

一個 10Y IRS 的 cash flows 從第 1 年到第 10 年均有付款。如果只 shift 第 10 年的 zero rate，所捕捉的敏感度只是年金因子的一小部分（roughly $\delta_{last} \times D(t_{10})$，而非 $\sum_{i=1}^{20} \delta_i D(t_i)$）。

用這個**嚴重低估**的 PV01 來正規化，會把 10Y IRS 的殘差**放大** 10–15 倍（相對於短天期商品），產生與 Gemini 在 answer_1 中聲稱「消除量綱失衡」完全相反的效果。

**問題三：parallel shift DV01 雖然比 single-pillar 正確，但仍非最合適。**

Parallel shift DV01 = $\frac{\partial NPV_i}{\partial r_{parallel}}$，對 par 商品與 annuity-based PV01 近似相等，但計算成本較高（需要重建整條 curve）。正確作法是直接解析式計算 annuity factor，而非透過任何 curve shift。

### 結論（立場更新）

$Market\_PV01_i$ 應該定義為**商品 i 對自身 par rate 的 annuity-based PV01**，解析式計算，不涉及任何 curve shift（無論 single-pillar 或 parallel）。

Gemini answer_4 中說「Market_PV01 作為校準開始前的常數，不會增加 LM 迭代內部的負擔」——我認同這個特性，但原因應該是「可解析計算」而非「numerical shifting with static freeze」。

---

## 二、ε 門檻的合理性：攻擊 fallback to 1.0 的邏輯

### Claude answer_4 的建議

我在 answer_4 中建議：當 $Market\_PV01_i < \epsilon = 1e{-8}$ 時，fallback 到 1.0（即不縮放）。我現在認為這個建議有兩個錯誤。

### 錯誤一：ε = 1e-8 是死碼（dead code）

對 notional = 1 的商品，解析式 PV01 的下界：

| 商品 | 估算 PV01 下界 |
|---|---|
| O/N Deposit (τ ≈ 1/252) | ≈ 4 × 10⁻⁷ |
| 1W Deposit | ≈ 2 × 10⁻⁶ |
| 最短期 IRS（1Y, quarterly） | ≈ 4 × 10⁻⁵ |

在任何正常市場環境下，notional ≥ 1 的商品的解析式 PV01 永遠不會低於 1e-7，更不會低於 1e-8。**ε = 1e-8 的門檻在實際校準中從來不會觸發，是名副其實的死碼。**

若採用解析式計算（而非 numerical bumping），出現 PV01 ≈ 0 的唯一情況是 notional = 0 或 annuity ≈ 0（兩者都是資料錯誤，應在資料載入時就攔截）。

### 錯誤二：fallback to 1.0 在概念上引入新的量綱問題

假設某個商品 $i$ 觸發了 fallback：$PV01_i \approx 0$，fallback 後 $f_i = NPV_i / 1.0 = NPV_i$。

此時：
- 其他商品：$f_j = NPV_j / PV01_j$，量級約為「利率偏差（basis points）」，即 $O(10^{-4})$ 到 $O(10^{-2})$
- 觸發 fallback 的商品 $i$：$f_i = NPV_i$，量級取決於商品的 NPV（對 notional = 1 的商品約為 $O(10^{-4})$，但若 notional = 1e6 則為 $O(100)$）

用 fallback 沒有保留任何量綱一致性——它只是表面上避免了除以零，卻把一個不可比的殘差混入了已正規化的殘差向量。正確的處理是：**把 PV01 ≈ 0 視為輸入資料錯誤並回傳錯誤**，不做 fallback。

### 更新後的建議

```rust
fn compute_market_pv01(helper: &InterestRateCurveCalibrationHelper, curve: &dyn InterestRateCurve) -> Result<f64, CalibrationError> {
    let pv01 = helper.instrument().analytic_pv01(curve)?;  // 解析式年金因子
    if pv01 < 1e-12 {
        return Err(CalibrationError::DegenerateInstrument {
            // 零 PV01 是資料錯誤，不做 fallback
        });
    }
    Ok(pv01)
}
```

門檻改為 1e-12（純粹的數值保護，防止 IEEE754 下溢），絕不 fallback to 1.0。

---

## 三、ShiftZeroRateCurve 設計：攻擊最弱假設

### 最弱假設

`ShiftZeroRateCurve` 設計中最弱、最危險的假設是：

**`shift_pillar: u32` 能夠明確識別「哪個 zero rate 對哪個商品是最相關的」。**

這個假設在以下情況失效：

**失效一：u32 pillar index 與商品的 par rate 無一對一對應。**

curve 的 pillar 是按日期排列的節點（例如 1Y, 2Y, ..., 30Y）。一個 5Y IRS 並沒有一個「專屬的」pillar——它的現金流横跨多個 pillar 區間。如果選 shift_pillar = 5（5Y pillar），你計算的是「5Y zero rate 對這個 IRS 的 sensitivity」，而非這個 IRS 對「自身 par rate」的 sensitivity。兩者在 par 附近近似，但對有 stub 或非標準 tenor 的商品可能差距顯著。

**失效二：PV01 計算與 LM 的參數空間脫鉤。**

LM 的參數 $x_j$ 是**插值空間**中的值（`InterpolationTarget` 決定：LogDiscount、ZeroRate、或 InstantaneousForwardRate）。Jacobian 的每一列是 $\partial NPV_i / \partial x_j$（對插值空間的偏導）。

`ShiftZeroRateCurve` 計算的是 $\partial NPV_i / \partial (\text{zero rate at pillar } k)$。若 LM 工作在 LogDiscount 空間，這個量與 Jacobian 列向量的量綱不同（差一個 $t_k$ 的因子），用它作正規化器會引入與 maturity 成正比的系統性偏差——長天期商品的殘差被低估縮放，短天期被過度縮放。

**失效三：`to_inst_forward_curve` 的 finite difference 精度問題。**

```rust
fn to_inst_forward_curve(&self) -> Arc<dyn DiscountCurve> {
    // 利用 discount 的 finite difference 反推
}
```

Instantaneous forward rate 的 finite difference 估計對步長 $h$ 非常敏感：
- $h$ 過大：差分誤差大
- $h$ 過小：浮點消除誤差

更重要的是：`ShiftZeroRateCurve` 在一個 pillar 上引入的 1bp 零利率 bump，對應的 inst forward rate 是一個 hat function（在 bump pillar 兩側的有限差分中出現）。這個 hat function 的高度由 $h$（finite difference 步長）決定，而非由物理上合理的瞬時遠期利率結構決定。PV01 對這個 $h$ 的選取敏感，不是穩健的計算方式。

### 替代建議：解析式 annuity PV01

完全不需要 `ShiftZeroRateCurve`。在 `Instrument` trait 上新增：

```rust
fn analytic_pv01(&self, curve: &dyn InterestRateCurve) -> Result<f64, PricerError>;
```

實作：

```rust
// Deposit
fn analytic_pv01(&self, curve: &dyn InterestRateCurve) -> Result<f64, PricerError> {
    let discount = curve.to_discount_curve();
    let tau = self.day_count().year_fraction(self.start_date(), self.end_date());
    Ok(tau * discount.discount(self.payment_date()) * self.notional().abs() * 1e-4)
}

// InterestRateSwap（使用固定腳年金因子）
fn analytic_pv01(&self, curve: &dyn InterestRateCurve) -> Result<f64, PricerError> {
    let discount = curve.to_discount_curve();
    let annuity: f64 = self.fixed_leg()
        .payment_dates()
        .zip(self.fixed_leg().day_count_fractions())
        .map(|(t, delta)| delta * discount.discount(t))
        .sum();
    Ok(annuity * self.notional().abs() * 1e-4)
}
```

這個方案：
1. 計算成本極低（O(n) disc factor lookups，且 `PrecomputedDiscountCurve` 已存在可直接加速）
2. 精確到機器精度，無 finite difference 誤差
3. 不依賴任何 pillar 索引約定
4. 不需要引入新型別

---

## 立場匯整

| 議題 | 原始理解 | 我的修正立場 | 說明 |
|---|---|---|---|
| Market_PV01_i 定義 | Ray：single-pillar zero rate shift | **Annuity-based par PV01，解析式計算** | single-pillar shift 嚴重低估長天期商品敏感度 |
| ε = 1e-8 門檻 | Claude answer_4：可接受 | **修正為 1e-12 的純數值保護，不做 fallback** | 1e-8 是死碼；fallback to 1.0 引入新的量綱問題 |
| ShiftZeroRateCurve | Ray 提案 | **反對；改用 `analytic_pv01()` trait method** | shift_pillar: u32 無法正確對應商品的 par rate |

**立場變更**：撤回 answer_4 中對 ε = 1e-8 fallback 的建議——那個設計在理論上有根本缺陷，不是參數選取問題，而是 fallback 邏輯本身的問題。
