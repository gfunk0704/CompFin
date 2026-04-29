# Claude Answer 8 — LM 理解確認與 Total DV01 權重反駁

**閱讀範圍**：所有歷史討論紀錄（user_question_1–8、claude_answer_1–7、gemini_answer_1–7）、lm.pdf、gradient_descent.pdf

---

## 一、Levenberg-Marquardt 理解確認

### 1.1 理解正確的部分

- **移動方向**：往梯度為零的方向移動，這是 descent method 的核心，正確。
- **λ 的定性作用**：λ 大時更保守（GD-like），λ 小時更激進（GN-like），這個定性理解正確。

### 1.2 需要更正的部分——共三點，其中第一點為根本性錯誤

---

**第一點（根本性錯誤）：Least-square 問題不一定是 convex function**

Ray 的原話：「因為 least-square problem 是一個 quadratic function，因此 objective function 也是 convex function」

這個推論只在模型 $\hat{y}(t; \mathbf{c})$ **對參數 c 是線性的**時候成立。在那種情況下：

$$\chi^2(\mathbf{c}) = \|y - X\mathbf{c}\|^2_W$$

確實是 **c** 的二次函數，也確實是 convex。

但是 lm.pdf 的標題明確說的是「**nonlinear** least squares」。當 $\hat{y}(t; \mathbf{c})$ 對 **c** 是非線性的，$\chi^2$ 一般既不是二次函數，也不是 convex。lm.pdf Section 6 的 Example 3 圖 3(a) 明確展示目標函數 "clearly not quadratic and has multiple minima"。

在我們的曲線校準問題中，NPV 對 zero rate 的依賴是透過折現因子 $e^{-r(T) \cdot T}$，這是對 **r** 非線性的。因此我們的問題是**非線性最小平方問題，objective function 不保證是 convex**。

這個區別有直接的實作意義：non-convex 的 objective 可能有多個局部最小值（QuantLib GlobalBootstrapper 正是為此設計了 trust-region 方法），初始猜測的選擇對是否收斂到全局最小值有決定性影響。

---

**第二點（理解偏差）：Gauss-Newton 不是用 "exact line search" 找 step size**

Ray 的描述：「傳統 Gauss-Newton method 是在一階 Taylor expansion 中用類似 GD 的 exact line search 的方式去找 step size」

這是錯的。GN 與 GD 的 exact line search 是本質上不同的操作。

- **GD exact line search**：沿著梯度方向做一維最小化，$t_k = \arg\min_{t>0} f(x_k - t\nabla f(x_k))$。這是一個 1D 優化問題。

- **Gauss-Newton**：對 $\hat{y}(\mathbf{c}+\mathbf{h}) \approx \hat{y}(\mathbf{c}) + J\mathbf{h}$ 做一階 Taylor 展開，代入 $\chi^2$ 得到 $\mathbf{h}$ 的**二次函數**，令其梯度為零求解 normal equations：

$$[\mathbf{J}^T \mathbf{W} \mathbf{J}]\, \mathbf{h}_{gn} = \mathbf{J}^T \mathbf{W} (\mathbf{y} - \hat{\mathbf{y}})$$

這是解一個**線性方程組**，不是沿某個方向做 line search。類比應該是 GD 中「使用 Newton step（利用二階曲率資訊）」，而不是「exact line search（沿梯度方向的 1D 最小化）」。

---

**第三點（描述混淆）：LM 的 λ 機制不是「試試看小的 step size 後再擴大搜尋範圍」**

Ray 的描述更像是 GD notes 中 **backtracking line search** 的邏輯（初始 t=1，條件不滿足就乘以 β 縮小）。

LM 的實際機制是：

1. 求解 $[\mathbf{J}^T \mathbf{W} \mathbf{J} + \lambda\, \text{diag}(\mathbf{J}^T \mathbf{W} \mathbf{J})]\, \mathbf{h}_{lm} = \mathbf{J}^T \mathbf{W} (\mathbf{y} - \hat{\mathbf{y}})$，一次得到候選步伐 $\mathbf{h}$
2. 計算 gain ratio $\rho$（實際改善 vs 近似改善的比值）
3. 若 $\rho > \epsilon_4$（步伐夠好）：**接受**此步，**減小** λ（下一步更 GN-like，更激進）
4. 若 $\rho \leq \epsilon_4$（步伐太差）：**拒絕**此步，**增大** λ（下一步更 GD-like，更保守）

關鍵在於：λ 的調整是改變「信任二次近似的程度」，而不是在某個固定方向上縮放步長。backtracking 是沿固定方向縮小 t；LM 是透過 λ 在整個 GD vs GN spectrum 之間移動。

---

## 二、攻擊 Total DV01（BBG spread curve 方法）的最弱假設

### 2.1 目標假設

Ray 論證中最弱的假設是：

> **「若使用 BBG 中計算 DV01 的方式（加上一個 spread curve）則所有商品可以一體適用」**

這個「一體適用」的宣稱是最弱的，因為它只需要一個反例就能被推翻，而反例恰好是我們系統中最常見的商品類型之一。

### 2.2 BBG spread curve 方法的數學實質

「加上一個 spread curve」= 對整條曲線做平行位移 +1bp，重算 NPV，取差分：

$$\text{Total DV01}_i \approx \frac{NPV_i(r + 1\text{bp}) - NPV_i(r)}{1\text{bp}} = \sum_j \frac{\partial NPV_i}{\partial r_j} \cdot 1\text{bp}$$

這就是**有符號的 Parallel Shift DV01**——正是我在 claude_answer_7 中已經論證為有根本性缺陷的方案。

### 2.3 論證：「一體適用」在浮動腿商品上系統性失效

考慮一個 basis swap（floating vs floating + spread），例如 3M OIS vs 6M IBOR + spread：

- Pay leg（3M OIS）：對整條折現曲線平行上移，forward rates 上升 → pay leg 現值增加 → $\sum_j \partial NPV/\partial r_j|_{pay} > 0$
- Receive leg（6M IBOR + spread）：同樣的曲線平行上移 → forward rates 上升 → receive leg 現值也增加 → $\sum_j \partial NPV/\partial r_j|_{receive} > 0$

如果兩條腿參照同一折現曲線，兩個貢獻方向相反（一正一負）。對一個接近 at-market 的 basis swap，這兩個貢獻幾乎精確抵消：

$$\text{Total DV01}_i = \sum_j \frac{\partial NPV_i}{\partial r_j} \approx 0$$

此時用這個 $\omega_i \approx 0$ 作為縮放因子，校準器的殘差 $f_i = NPV_i / \omega_i$ 趨近無窮大，或觸發數值崩潰。這比 claude_answer_5 中批評的 ε fallback 問題更嚴重——ε fallback 至少給了一個固定下界；BBG spread-curve 給出的接近零的數值是由商品的現金流結構決定的，不能用 clamping 修補，因為 $\omega_i \approx 0$ 正是這類商品的「正確」的 parallel DV01。

**「一體適用」的宣稱在碰到帶有浮動腿的商品時系統性失效，而 basis swap 正是利率曲線校準中最常見的工具之一。**

### 2.4 附帶論證：「只有一開始進行一次」也是錯的

Ray 說 Total DV01 計算「只有一開始需要進行一次」。

這個宣稱在以下情況下不成立：

**情況 A（固定初始值）**：如果把初始猜測時計算的 $\omega_i$ 固定使用到收斂，隨著曲線在迭代中移動，預條件化的品質會逐漸退化。對非線性問題（如上文指出，我們的問題是非線性的），初始猜測可能離解很遠，此時初始 $\omega_i$ 對描述解附近的 landscape 毫無代表性。

**情況 B（每次迭代更新）**：如果每次迭代重算 $\omega_i$（以反映最新的曲線狀態），則需要對每個商品做一次 NPV 重算，每次迭代的額外計算量是 O(m) 次 NPV 計算（m 為商品數）。這與 par PV01 的解析式計算（O(m) 次 annuity 加總）相比，前者的計算量更大，而不是「只有一次」。

### 2.5 附帶論證：架構問題被高估了

Ray 說 par rate PV01 需要修改架構（QuoteSheet / generator 分離問題）。

在 claude_answer_7 的 Solution B 中已經指出：par PV01 可以在 `InterestRateCurveCalibrationHelper` 的生成階段計算並儲存，因為那個層次同時持有 instrument（含現金流結構）和 market_rate（含 quote_target）。不需要讓 generator 知道 quote 型態，也不需要讓 QuoteSheet 知道 generator 的實作細節。架構不需要改變。

---

## 三、立場聲明

**我不改變立場。**

Gemini 的「一體適用」論點在遇到浮動腿商品時失效，這不是邊緣案例而是系統核心產品（OIS basis swap 是折現曲線校準不可避免的工具）。Ray 的「計算只需一次」假設在嚴格分析下也不成立。

**維持 claude_answer_5/6/7 的立場：Par PV01（$N \cdot \sum_k \delta_k \cdot D(T_k) \times 1\text{bp}$）是最合適的縮放因子。**

其理由的優先順序：

1. **正確的金融量綱**：縮放後的殘差是「利率空間誤差」，對所有期限商品的 1bp 誤差一視同仁
2. **不引入 duration 偏差**：不系統性壓制長端殘差
3. **不存在代數抵消崩潰**：Par PV01 是對 par rate 的偏導，不涉及浮動腿與固定腿之間的相殺
4. **解析式可計算**：O(m) annuity 加總，不需要曲線擾動重算
5. **架構問題已解決**：Solution B 在 CalibrationHelper 層計算 PV01，無需改動 QuoteSheet 或 generator

---

## 四、立場匯整

| 議題 | Ray 的問題 | 我的立場 |
|---|---|---|
| LM convexity | Least-square 一定 convex？ | **錯**：只有線性模型才保證 convex；非線性 least squares 可有多局部最小值 |
| GN step size | GN 用 exact line search？ | **錯**：GN 解 normal equations（線性代數），不是沿梯度方向做 1D 最小化 |
| LM λ 機制 | 試小步長後擴大搜尋範圍？ | **不精確**：是在 GD/GN spectrum 之間調整，不是 backtracking；λ 增大 = 更 GD-like，不是「縮小搜尋範圍」 |
| Total DV01 通用性 | BBG spread curve 一體適用？ | **反對**：signed parallel DV01 在浮動腿商品上因代數抵消趨近零，通用性宣稱失效 |
| Total DV01 計算成本 | 只需算一次？ | **不正確**：固定初始值則預條件退化；每次更新則成本不低於 par PV01 |
| 架構問題 | Par PV01 需改架構？ | **已解決**：Solution B 在 CalibrationHelper 層計算，無需架構變動 |
