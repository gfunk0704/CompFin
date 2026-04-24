# Claude Answer 3 — 釐清細節 + 延伸問題

**閱讀範圍**：`user_question_1/2/3.md`、`claude_answer_1/2.md`、`gemini_answer_1/2.md`

---

## 一、釐清部分：`PillarAlignment` 進入 trait

### 接受 Ray 的設計決策

Ray 的設計消解了我在 answer_1/2 中提出的核心反對意見。我在 answer_2 中攻擊 Gemini 的立場時，論點是「`IterativeBootstrapper` 忽略參數導致靜默合約違反」。Ray 的方案實質上把 `pillar_alignment()` 從可配置參數改成**回報行為的查詢方法**：

```rust
// trait 定義
fn pillar_alignment(&self) -> PillarAlignment;

// IterativeBootstrapper 強制回報（不可配置）
fn pillar_alignment(&self) -> PillarAlignment {
    PillarAlignment::MaxDate
}
```

這不是「允許外部設定 `MaturityDate`」，而是「讓呼叫者知道這個 calibrator 使用 `MaxDate`」。我在 answer_2 中反對的是前者，Ray 的設計是後者。立場調整：**支持這個設計**。

### 仍需解決的實作問題

**問題一：`Instrument` trait 需要 `maturity_date()`**

`PillarAlignment::MaturityDate` 要提取「到期日」，但目前 `Instrument` trait 只有 `max_date()`。`LeastSquareCalibrator` 若要使用 `MaturityDate`，需要在 `Instrument` 上新增：

```rust
fn maturity_date(&self) -> NaiveDate;
```

建議的 `PillarAlignment` 設計：

```rust
pub enum PillarAlignment { MaturityDate, MaxDate }

impl PillarAlignment {
    pub fn extract_date(&self, instrument: &dyn Instrument) -> NaiveDate {
        match self {
            PillarAlignment::MaxDate      => instrument.max_date(),
            PillarAlignment::MaturityDate => instrument.maturity_date(),
        }
    }
}
```

**問題二：`generate_calibration_set` 不受影響**

確認：`generate_calibration_set` 目前是靜態方法（無 `&self`），不提取 pillar dates，只建立 `InterestRateCurveCalibrationHelper`。Pillar date 提取發生在各自的 `calibrate()` 實作中。因此這個靜態方法不需要修改。`calibrate()` 的實作各自呼叫 `self.pillar_alignment().extract_date(h.instrument())` 即可。

### 關於 MX 一次全域求解的說明

Ray 提到 MX 使用 multivariate Newton 的全域求解性質，這也直接說明了為什麼 `LeastSquareCalibrator` 可以合理支援 `MaturityDate`：全域方法同時調整所有 pillar 值，即使部分現金流落在最後一個 pillar date 之外（外插區域），solver 仍然可以透過統一調整所有 pillars 來把這些流量的 NPV 貢獻考慮進去。這與逐點方法的結構性差異是根本的。我在 answer_1 中論證 `max_date` 是 `IterativeBootstrapper` 的「結構性約束」，這個論點對 `IterativeBootstrapper` 成立，但對全域求解方法不成立。

---

## 二、延伸問題一：方形系統中 Par Rate 的必要性

**結論：不值得實作，對這個系統無實質益處。**

### 攻擊 Gemini 在 answer_1 中對 par rate 的主張

Gemini 在 answer_1 中建議：「最穩健的做法是最小化**利率偏差** $(ModelRate - MarketRate)^2$ 或將 NPV 除以 PV01 進行標準化」、「直接計算該商品在當前曲線下的平價利率並與市場報價相減。」

Gemini 的最弱假設是：**「Par rate 殘差比 raw NPV 殘差在數值上更優越。」**

這個假設是錯的，而且 QL 使用 par rate 解析解的動機與 Gemini 的理由完全不同。

#### 為什麼 par rate ≡ NPV = 0

對任何 par 計價商品（deposit 或 IRS），定義上：

$$
\text{par rate 條件：} NPV(r^*) = 0
\iff
r^* = r^{market}
$$

「par rate = market rate」與「NPV = 0」是**同一個代數方程的兩種寫法**。切換到 par rate 殘差不改變解的位置，也不改變任何收斂性質。Gemini 的論點隱含了一個額外假設：par rate 殘差的 Jacobian 條件數比 NPV 殘差好。這在方形系統中需要嚴格論證，而 Gemini 沒有提供。

#### QL 使用 par rate 的真正動機

QL 的 iterative bootstrapper 在每次求解第 i 個 pillar 時，不想對一個「完整的 NPV 計算流水線」做 root solve（因為 QL 的流水線很重，有多層 lazy 計算和 observer pattern）。QL 的解法是：對固定利率 IRS，par rate 有解析式：

$$
r^{par} = \frac{P(t_0) - P(t_n)}{\sum_{i} \delta_i \cdot P(t_i)}
$$

這讓他們在不走完整 NPV 計算的情況下直接求解。**這個優化與「par rate 殘差精度更好」無關，而是為了繞過 QL 架構中的計算成本。**

本專案的 `SimpleInstrumentPricer` 已經是最短路徑——沒有 observer pattern 開銷，沒有 lazy 計算層。QL 的動機在這裡完全不適用。

#### Compounding IRS 沒有 par rate 解析式

Ray 問的核心場景：**compounding IRS + lookback + lockout。**

在 `CompoundingRateIndex::standard_forward_factor()` 中：

```rust
let rate = (discount_curve.discount(fixing) / discount_curve.discount(next_fixing) - 1.0) / tau;
```

浮動腳的 compound factor 是逐日迭代計算的，且 `accrual_to_fixing()` 的 lockout/lookback 邏輯讓每天的 fixing date 獨立映射。這個結構不能化簡為折現因子的封閉式表達式。想要求「par rate」，就必須先跑完這個逐日迭代，再做一個 root solve 找讓 fixed = floating 的固定利率。這是在 NPV 計算**外面再套一層 root solve**，計算成本翻倍以上，且最終還是等價於 NPV = 0。

**Ray 的 Deposit（BondHelper-like）情況類似**：即使結構比 compounding IRS 簡單，par rate 解析式仍然依賴精確的 day count convention 和 compounding 方式。這些在 Ray 的系統中是 generator 層級的參數，通用 par rate 公式不存在。

#### 結論

par rate 解析解在此系統中：(1) 對方形系統無數值優勢；(2) 不適用於 compounding IRS with lookback/lockout；(3) 對 Deposit 即便可寫出，也只是 NPV = 0 的等價表達；(4) 實作複雜度高，帶來維護成本。**不引入。**

---

## 三、延伸問題二：`PrecomputedDiscountCurve` 在 `LeastSquareCalibrator`

**結論：值得引入，但受益對象主要是含 `CompoundingRateIndex` 的校準集，且實作需要一個 wrapper 型別。**

### 受益程度分析

`CompoundingRateIndex::standard_forward_factor()` 的每個 calculation period 迭代所有 accrual days：

```
一個 3M SOFR 期間 ≈ 65 個業務日 → 65 次 to_discount_curve().discount(date) 呼叫
```

一個 10Y SOFR compounding IRS 的浮動腳：~40 個 quarterly periods × ~65 天 = **2600 次 discount() 呼叫**（每次 LM 迭代，就這一個商品而言）。

在 `LeastSquareCalibrator` 中，所有 n 個校準商品使用**同一條** trial curve。若 10 個 IRS 商品中前 5 個的 daily discount dates 有大量重疊（例如前 1Y 的每個業務日），`PrecomputedDiscountCurve::Dense` 的效益在數量級上：

| 情境 | discount() 呼叫次數（每次 LM 迭代） |
|---|---|
| 無 precompute | n × m × k（可能 > 10,000） |
| 有 precompute (Dense) | D unique dates（可能 < 2,500） + D 次 Vec index |

且 `CacheStrategy::Dense` 的 Vec index 是 ~7ns，遠快於 `PiecewisePolynomial` 的插值計算（需要 binary search + 多項式求值）。

### 實作障礙與解法

**障礙**：`PrecomputedDiscountCurve` 實作的是 `DiscountCurve` trait，但 `CompoundingRateIndex` 接受的是 `Arc<dyn InterestRateCurve>`，並在內部呼叫 `forward_curve.to_discount_curve()`。需要一個 wrapper：

```rust
pub struct CurveWithPrecomputedDiscount {
    underlying:           Arc<dyn InterestRateCurve>,
    precomputed_discount: Arc<PrecomputedDiscountCurve>,
}

impl InterestRateCurve for CurveWithPrecomputedDiscount {
    fn to_discount_curve(&self) -> Arc<dyn DiscountCurve> {
        Arc::clone(&self.precomputed_discount)
    }
    fn to_zero_rate_curve(&self) -> Arc<dyn ZeroRateCurve> {
        self.underlying.to_zero_rate_curve()
    }
    fn to_inst_forward_curve(&self) -> Arc<dyn InstForwardCurve> {
        self.underlying.to_inst_forward_curve()
    }
    fn year_fraction_calculator(&self) -> &YearFractionCalculator {
        self.underlying.year_fraction_calculator()
    }
}
```

### 建議的一次性初始化

校準開始前（非每次迭代）：

1. 遍歷所有校準商品，對每個 calculation period 呼叫 `index.relative_dates_for_period(period)` 收集所有 unique daily discount dates（`CompoundingRateIndex::relative_dates_for_period` 已實作）
2. 判斷是否使用 `Dense` 策略：若最大 span ≤ N 年且 daily fill rate > 20%（沿用 `CacheStrategy::Auto` 邏輯）

每次 LM 迭代：

```rust
let discount_curve = trial_curve.to_discount_curve();
let precomputed = Arc::new(PrecomputedDiscountCurve::new(
    discount_curve,
    &all_daily_dates,
    CacheStrategy::Dense { reference_date, max_days },
));
let effective_curve = Arc::new(CurveWithPrecomputedDiscount {
    underlying: trial_curve.clone(),
    precomputed_discount: precomputed,
});
// 用 effective_curve 替代 trial_curve 進行所有 NPV 計算
```

### 一個條件：Arbitrage-Free 路徑不受益

若 `CompoundingRateIndex` 開啟 `use_arbitrage_free = true`，整個 compound factor 縮減為 `D(start)/D(end)` 兩次 discount 呼叫，`PrecomputedDiscountCurve` 節省的邊際效益極小。Sparse 模式已夠用。這不影響整體建議，但可以在實作中加條件判斷：若全部浮動腳都走 arbitrage-free 路徑，跳過 Dense 初始化。

### 最終立場

值得引入，但有清楚的前提：校準集包含 `CompoundingRateIndex`（SOFR/TONAR compounding IRS）且不全走 arbitrage-free 路徑。純 term rate（LIBOR-style）或 arbitrage-free SOFR 的校準集中，效益較小，可以考慮 Sparse 或不啟用。
