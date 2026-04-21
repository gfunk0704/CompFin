# Claude 回覆 — LeastSquareCalibrator 設計分析

**閱讀範圍**：本資料夾現僅有 `user_question_1.md`，無其他討論紀錄。

---

## 一、Python 架構問題分析

### 整體評估：架構方向正確，但有三個具體問題需要處理

#### 1.1 Pseudo-code 中的兩個 bug

**Bug 1 — `getDate` lambda 兩個分支都回傳 `maturityDate`：**

```python
getDate = lambda instrument: instrument.maturityDate if self.pillar == PillarAllignment.MATURITY_DATE \
          else lambda instrument: instrument.maturityDate   # ← 應為 maxDate
```

`else` 分支應為 `instrument.maxDate`，否則 `PillarAlignment` 沒有任何作用。

**Bug 2 — `LogDiscount` 初始值缺少負號：**

```python
initialGuess = [r * dayCounter(horizon, d) for r, d in zip(initialGuess, dates)]
```

應為 `-r * dayCounter(horizon, d)`，才能與 `ln(D(t)) = -r * τ` 一致。
這與現有 `BootstrappingTrait::initial_value` 的實作一致，確認是 pseudo-code 的遺漏。

---

#### 1.2 `curveMap` 的 mutable closure capture：Rust 裡必須重構

Python 的 `objectFunction` 直接 mutate 外層 `curveMap`：

```python
def objectFunction(guess):
    curveMap[curveName] = rateCurveGenerator(guess)   # 外層 dict mutation
    ...
```

這個模式在 Rust 裡有兩個問題：

1. `argmin` 的 `Operator` trait 要求 `&self`（不可變），不允許 closure 以 `&mut` 方式捕獲外部狀態。
2. 即使繞過 borrow checker（用 `RefCell`），這也是競爭狀態的根源。

**正確的 Rust 做法**：沿用 `IterativeBootstrapper::build_market_data` 的模式，在每次 objective function 評估時建構局部 `HashMap<String, Arc<dyn InterestRateCurve>>`：

```rust
let market_data: HashMap<_, _> = instrument_curve_names
    .iter()
    .map(|name| (name.clone(), trial_curve.clone()))
    .collect();
```

局部 `HashMap` 的 allocation 成本相對於 NPV 計算可以忽略。

---

#### 1.3 `argmin` LM solver 的 Jacobian 問題

`scipy` 的 `'lm'` 方法預設使用有限差分 Jacobian（MINPACK `lmdif`），不需要使用者提供解析式。

`argmin` 的 `LevenbergMarquardt` 需要實作 `Jacobian` trait。有兩條路：

| 路徑 | 成本 | 風險 |
|---|---|---|
| 有限差分 Jacobian（用 `argmin` 的 `FiniteDifferenceJacobian`）| 低，行為接近 scipy | 每次 LM 迭代多 n 次 NPV 計算 |
| 解析式 Jacobian | 高，需對 pillar values 手動微分 | 錯誤難以偵測 |

建議：**有限差分 Jacobian**，行為對齊 scipy `lm`，與 Python 版本可直接比較。

另外，`argmin` 的 operator trait 是 struct-based（需 `impl Operator for SomeStruct`），不接受 closure。因此 objective function 需包裝成持有必要參考的 struct，類似：

```rust
struct LmObjective {
    instruments:       Vec<Arc<dyn SimpleInstrument>>,
    curve_generator:   Arc<dyn InterestRateCurveGenerator>,
    pillar_dates:      Vec<NaiveDate>,
    reference_date:    NaiveDate,
    pricing_condition: PricingCondition,
}
```

這些欄位都是 `Send + Sync`，沒有問題。

---

## 二、`PillarAlignment` 的放置位置

**我的立場：不應放入 `InterestRateCurveCalibrator` trait，應定義在 module level 作為自由 enum。**

### 核心論點

Ray 的提案假設：`PillarAlignment` 是所有 calibrator 共用的「設定旋鈕」，因此放在 trait 層級可以統一管理。

**我要攻擊的假設**：`IterativeBootstrapper` 限定用 `MaxDate` 是一個「偏好」，而不是一個「演算法約束」。

這個假設是錯的。

`IterativeBootstrapper` 使用 `max_date` 不是偏好，是**結構性約束**。原因如下：

逐點拔靴法的核心操作是：
> 解出第 i 個 pillar 的值後，將第 0..i 段曲線固定，再求解第 i+1 個 pillar。

這意味著**第 i 個 pillar date 必須是第 i 個商品所有現金流的最右邊界**。若使用 `maturity_date`，最後一期浮動利率的 tenor end date 或 payment date 可能超出最後 pillar date，導致該商品的部分現金流必須從曲線的外插區域計算，而外插區域的形狀由 `ExtrapolationMethod` 決定，不再受 pillar 值控制，根求解失去意義。

換句話說：`max_date` 是確保「每個商品的 NPV 完全由已解出的 pillars 決定」的**必要條件**，不是可調整的參數。

### 將 `PillarAlignment` 放入 trait 的代價

如果把 `PillarAlignment` 加進 `InterestRateCurveCalibrator`（例如作為方法或 generate_calibration_set 的參數），`IterativeBootstrapper` 必須實作這個介面但內部忽略它。這製造了一個**靜默的合約違反**：外部呼叫者設定 `PillarAlignment::MaturityDate` 傳給 `IterativeBootstrapper`，編譯不報錯，執行也不報錯，但行為毫無變化。這比「沒有這個旋鈕」更危險。

### 建議的架構

```rust
// module level — interestratecurvecalibrator.rs 或新的 pillaralignment.rs
pub enum PillarAlignment {
    MaturityDate,
    MaxDate,
}

impl PillarAlignment {
    pub fn extract(&self, instrument: &dyn SimpleInstrument) -> NaiveDate {
        match self {
            PillarAlignment::MaturityDate => instrument.maturity_date(),
            PillarAlignment::MaxDate      => instrument.max_date(),
        }
    }
}
```

- `LeastSquareCalibrator` 持有 `pillar_alignment: PillarAlignment` field
- `GlobalFlowCalibrator`（未來）同樣持有 `pillar_alignment: PillarAlignment` field
- `IterativeBootstrapper` 不持有此 field，直接呼叫 `instrument.max_date()`
- `InterestRateCurveCalibrator` trait 不提及 `PillarAlignment`

這樣的設計讓 `PillarAlignment` 的定義可以共享（減少重複），但不製造虛假的 trait 合約。

---

## 三、Rust 實作的整體適配性

Python 架構的核心邏輯可以直接移植，主要差異如下：

| Python | Rust 對應 |
|---|---|
| `least_squares(fun, x0, method='lm')` | `argmin::LevenbergMarquardt` + `FiniteDifferenceJacobian` |
| `curveMap[curveName] = ...`（closure mutation）| 每次評估建局部 `HashMap` |
| `rateCurveGenerator.setDates(dates)` | `curve_generator.generate_with_dates(ref_date, &dates, values)` |
| `result.status <= 0` 錯誤判斷 | `argmin::TerminationReason` 匹配 |
| `instrumentList` 直接傳入 | 透過 `generate_calibration_set` 從 pillars 建構（沿用現有 trait 介面）|

`calibrate` 的方法簽名可以完全沿用現有 `InterestRateCurveCalibrator::calibrate` 的介面，不需要修改 trait。
