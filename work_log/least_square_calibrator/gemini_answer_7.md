# Gemini Answer 7 — 權重與產品定義的深度解析

**閱讀範圍**：所有歷史討論紀錄（1–6 輪），重點關注 `user_question_7`。

---

## 一、 核心立場：對 Claude Answer 6 的技術反駁

根據指示，我將針對 Claude 在 `claude_answer_6` 中的論點進行攻擊，並說明我的立場。

### 1.1 攻擊 Claude 的最弱假設：Annuity-based PV01 的「通用性」
Claude 堅持 $Market\_PV01$ 必須是「解析式的 Annuity-based Par PV01」，其核心假設是：**所有校準工具（Instruments）都能定義一個唯一的 Par Rate 且其敏感度由 Annuity 主導。**

這個假設在處理使用者提到的 **「包含期初本金支付的 Deposit」** 與 **「Basis Swap」** 時會遇到嚴重的技術困難：

1.  **Deposit 的「量綱爆炸」問題**：
    使用者定義的 Deposit 包含期初本金（-100% at $T_0$）與期末本金+利息（+100%+r at $T$）。
    *   對於一個 1-Day Deposit，其 par PV01 (Annuity) 約為 $1/360 \approx 0.0027$。
    *   但該產品對 $T_0$ 和 $T$ 的折現因子敏感度接近 $1.0$。
    *   若依 Claude 建議除以 $0.0027$，殘差會被放大 **360 倍**。這會導致校準器過度優化這極短端的 1bp 誤差，而忽視了長端長達 30 年的殘差。這不是「正規化」，這是「雜訊放大器」。

2.  **Basis Swap 的定義模糊**：
    在 Basis Swap（Float vs Float + Spread）中，Annuity 是對 Spread 的敏感度。然而，校準時我們可能正在解兩條不同的曲線（Discount vs Forward）。只用 Spread Annuity 作為 Scaling，忽略了兩條腿浮動利率部分的敏感度，這在數值優化上是不完整的。

### 1.2 我是否改變立場？
**是的，我部分改變立場。**
我不再堅持 `Single-pillar Sensitivity`（Gemini 5/6 方案），但我也不接受 Claude 的 `Annuity` 方案。我認為使用者提出的 **「Total DV01（所有 Pillar 敏感度之和）」** 是一個在數學與金融實作上更完美的折衷方案。

---

## 二、 對使用者問題的技術回覆

### 2.1 關於權重：支持「Total DV01」方案
使用者提議使用各 Pillar DV01 的總和：$\omega_i = \sum_{j=1}^{N} \frac{\partial NPV_i}{\partial r_j}$。

我全力支持這個方向，理由如下：
1.  **自動適應產品特性**：對於包含本金的 Deposit，它會捕捉到本金流的敏感度（~1.0）；對於 Swap，它會捕捉到整體年金的敏感度。這完美解決了上述 Claude 方案在 Deposit 上的縮放失衡問題。
2.  **單位一致性（Rate Space）**：將 $NPV$ 除以「對利率參數的總敏感度」，其結果的單位依然是「利率誤差（Rate Error）」，這達成了 Claude 追求的金融直覺。
3.  **數值魯棒性**：它比 Single-pillar（Gemini 方案）更穩定，不會因為 Instrument 與 Pillar 的相對位置（邊界問題）而導致敏感度驟降。

**建議公式修正**：
為了防止正負敏感度抵消（雖然在標準產品中少見），建議使用：
$$ \omega_i = \sum_{j=1}^{N} \left| \frac{\partial NPV_i}{\partial r_j} \right| $$

### 2.2 關於名目本金（1,000,000）與 $\epsilon$ 門檻
即使預設名目本金很大，**$10^{-12}$ 的門檻依然絕對必要**。
*   **理由**：這不是針對市場報價的保護，而是針對 **「程式碼健壯性」** 的保護。
*   **情境**：如果使用者不小心輸入了 `nominal = 0`，或者 `start_date == maturity_date` 導致沒有現金流，沒有門檻檢查會導致 `1.0 / 0.0` 觸發程序崩潰。
*   **結論**：維持 $10^{-12}$ 檢查。若小於此值，應報錯 `CalibrationError::InsufficientSensitivity` 而非靜默處理。

### 2.3 產品定義：Deposit 與 Basis Swap
1.  **Deposit**：您的定義（包含期初本金）與 Murex 的 `Deposit` 行為高度一致。這在校準 Discount Curve 的 $T=0$ 點時非常有用。
2.  **Basis Swap**： par rate 計算並不複雜。在 `LeastSquareCalibrator` 中，我們直接最小化 $NPV(x)$。 par spread $s$ 只是一個參考值。
    $$ s_{model} = \frac{Leg_{float, 1} - Leg_{float, 2}}{Annuity_{spread}} $$
    校準殘差依然可以定義為 $f_i = (s_{model} - s_{market}) \times \text{Weight}$。

---

## 三、 行動建議與總結

### 3.1 關於 `PillarAlignment` 與 `maturity_date`
*   **同意實作**：在 `Instrument` trait 中新增 `maturity_date()`。
*   **Deposit 實作**：回傳 `schedule.last_period().payment_date()`（這與您的敘述一致）。
*   **IRS 實作**：回傳兩條腿中最晚的 `payment_date()`。

### 3.2 最終權重策略建議
在 `LeastSquareCalibrator` 中提供兩種選擇：
1.  **None**：權重為 1.0 (金額空間)。
2.  **TotalDV01** (預設)：$\sum | \partial NPV / \partial r_j |$ (利率空間，通用性最強)。

這套方案避開了 Claude 的「過度依賴產品定義」與 Gemini 的「過度依賴網格密度」，是最符合 `CompFin` 企業級架構要求的。
