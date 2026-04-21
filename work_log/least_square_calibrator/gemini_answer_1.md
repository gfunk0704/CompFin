# Gemini Answer 1 - LeastSquareCalibrator 實作討論

## 背景檢視與推導

根據 `./work_log/least_square_calibrator/user_question_1.md` 中的描述，目標是實作一個能對齊 Murex (MX) 行為的 `LeastSquareCalibrator`。MX 的核心特徵在於其「全域求解」的本質，這與 `IterativeBootstrapper` 逐點前進的邏輯有顯著差異。

### 1. 架構適合 Rust 實作嗎？

**結論：非常適合。**

- **優化庫：** 專案已引入 `argmin` 與 `argmin-math`。`argmin` 的 `LevenbergMarquardt` 實作者（通常配合 `nalgebra`）能完美對應 Python `scipy.least_squares(method='lm')`。
- **目標函數：** Rust 的閉包 (closure) 或結構體實作 `ArgminOp` trait 可以輕鬆封裝 `curve_generator` 與 `pricer.market_value` 的呼叫。
- **類型系統：** `InterestRateCurveCalibrator` trait 已經定義了 `calibrate` 介面，`LeastSquareCalibrator` 只需要實作該 trait，並將 `initialGuess` 與全域目標函數封裝進 `argmin` 的執行流程即可。

### 2. 最弱假設論證

在提供的 Python 虛擬碼與設計中，**「以原始市場價值 (Market Value, NPV) 作為 Objective Function 的殘差項」** 是最弱且風險最大的假設。

**論證如下：**

- **量綱與敏感度失衡 (Scaling/Sensitivity Mismatch)：** 
  Levenberg-Marquardt (LM) 演算法最小化的是殘差平方和 $\sum f_i(x)^2$。在利率曲線校準中，不同長度的商品其 PV01 (Price Value of a Basis Point) 有量級上的差異。
  - 一筆 30Y IRS 的 NPV 對利率變動的敏感度（PV01）可能是一筆 3M Deposit 的數百倍。
  - 若直接最小化 NPV，優化器會發現：僅僅移動 30Y 的點就能劇烈改變總殘差，而移動 3M 的點幾乎沒影響。這導致優化器會「過度照顧」長端品種，而容忍短端品種存在極大的利率偏差（Quotes Error）。
- **數值穩定性：**
  Murex 或 QuantLib 的全域校準通常不會直接使用 $NPV$。更穩健的做法是最小化 **利率偏差** $(ModelRate - MarketRate)^2$ 或者將 NPV 除以該商品的 $PV01$ 進行標準化（使其變成無因次的 Basis Point 誤差）。
- **Python 虛擬碼的潛在 Bug：**
  虛擬碼中 `initialGuess` 對於 `LOG_DISCOUNT` 的處理為 `r * dayCounter`。在我們的 `BootstrappingTrait` 中，為了符合 $D(t) = \exp(-r \cdot \tau)$，使用的是 `-market_rate * tau`。正負號與符號慣例需在實作時對齊，否則 LM 會直接發散。

### 3. PillarAllignment 的位置與必要性

**關於 `PillarAllignment` (Maturity vs Max Date)：**

- **現狀：** `IterativeBootstrapper` 強制使用 `max_date()`。這是因為在逐點拔靴中，若 pillar point 未能覆蓋該商品「最遠」的現金流（例如 Floating Leg 的最後一個 fixing end date 通常晚於 Maturity Date），會導致曲線在最後一小段發生不穩定的外插或未定義行為。
- **建議：** 我贊成將此列入 `InterestRateCurveCalibrator` 的配置參數，或者至少在 `LeastSquareCalibrator` 的建構子中開放。
- **理由：** 為了對齊 Murex，必須具備這種「刻意模擬對方行為」的自由度。MX 某些情況下確實只校準到 Maturity Date 而忽略了 tail flows 對曲線的微小影響。
- **實作細節：** 我們需要在 `Instrument` trait 中新增 `maturity_date()` 方法（目前只有 `max_date()`），或從 `SimpleInstrument` 向上轉型取得 `LegCharacters` 的 `maturity_date()`。

## 結論與立場

我**支持**引入 `LeastSquareCalibrator` 並採用 `argmin` 作為後端。

但我明確**反對**直接使用原始 NPV 作為殘差項。我建議在實作時，目標函數應改為：
$$ f_i(x) = \frac{NPV_i(x)}{PV01_i} $$
或者更簡單地，直接計算該商品在當前曲線下的 **平價利率 (Par Rate)** 並與市場報價相減。這能保證全曲線各個天期的校準精度在數值上具有相同的權重。

**立場改變：**
在檢視 Python 程式碼前，我曾傾向於維持 `max_date()` 的唯一標準。但在看到 MX 對齊需求後，我意識到「 pillar 日期與商品隱含日期的不匹配」本身就是一種市場慣例的體現，我們應該在 Calibrator 層級提供這個自由度，而非硬編碼在 Instrument 內。
