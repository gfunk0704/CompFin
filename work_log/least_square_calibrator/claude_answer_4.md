# Claude Answer 4 — 量比正規化確認 + PrecomputedDiscountCurve 細節

**閱讀範圍**：`user_question_1/2/3/4.md`、`claude_answer_1/2/3.md`、`gemini_answer_1/2/3.md`

---

## 主題一：Least Square 權重 / PillarAlignment

### 攻擊 NPV/Market_PV01 方案的最弱假設

Ray 採用了 Gemini 在 answer_3 的建議（與我在 answer_2 的立場直接衝突）。我的任務是找出這個方案中最弱的假設。

**最弱假設：Market_PV01 對 LM 殘差的縮放是在「利率空間」進行的，因此在整個 LM 迭代過程中有效。**

這個假設在以下情況下成立：對所有 i，NPV_i(x) ≈ PV01_i · (model_rate_i − market_rate_i)，也就是說 NPV 對利率偏差近似線性。然而這個線性近似的有效性依賴一個未被明說的前提：**trial curve 在迭代過程中始終接近市場水準**。

以下是這個前提在哪裡會失效：

**失效場景一：Jacobian 有限差分計算時。** LM 在計算 J 時使用有限差分：對每個 pillar k，擾動 x_k → x_k + h，重算所有 NPV。若 h 選取不當（過大或過小），x_k + h 可能遠離市場，此時 PV01(initial) 與 PV01(trial + h) 差異顯著，f_i = NPV_i(trial + h) / PV01_i(initial) 不再正確反映「利率偏差」，而是帶有量綱汙染的混合量。這不是 PV01 方案本身的問題，而是它的有效性依賴 h 的選取，而 h 的選取又依賴 NPV 的尺度——構成循環。

**失效場景二：特定商品的 Market_PV01 數量級異常小。** 若校準集包含一個到期日非常短的 Deposit（例如 O/N，τ ≈ 1/252），PV01 ≈ τ · D(T) ≈ 0.004。同一集合中有 30Y IRS，PV01 ≈ 150。正規化後，30,000 倍的殘差差距被消除，但在迭代初期若 O/N Deposit 的 NPV 因初始猜測稍差而相對偏大，正規化後的殘差會被極度放大，誤導 LM 優先校準短端而忽略長端。這與 Gemini 在 answer_3 聲稱「正規化消除量綱失衡」的方向恰好相反——錯誤的正規化可以放大相對偏差而非縮小它。

### 立場更新

我撤回 answer_2 中「方形系統不需要任何正規化」的強硬立場。Gemini answer_3 關於 Jacobian 行向量條件數的論點是正確的——即使在方形系統中，若 J 各行的 Frobenius 範數差距過大，LM 的收斂速度會退化。NPV/Market_PV01 確實將殘差近似對齊到利率空間（在初始猜測為市場利率的前提下），改善了 J^T J 的條件數。

但我的立場更新是有條件的：

1. **Ray 的使用場景**（initial guess = market rate 反推，系統始終在 at-money 附近求解）滿足線性近似的有效前提，Market_PV01(initial) ≈ Market_PV01(solution)，靜態縮放與動態縮放等效。接受這個方案。

2. **必須加保護**：在使用 Market_PV01 之前驗證 PV01 > ε（建議 ε = 1e-8），若某個商品的 Market_PV01 過小，fallback 到 1.0（即不縮放），並記錄警告。這個保護不是可選的——O/N Deposit 在某些 day count 慣例下確實可能出現極小 PV01。

### PillarAlignment 確認

Ray 的設計（包含 `extract_date` 方法的 enum）與 claude_answer_3 的建議完全一致，接受。

一個剩餘的實作問題值得明確：`PillarAlignment::MaturityDate` 分支呼叫 `instrument.maturity_date()`，但目前 `Instrument` trait 只有 `max_date()`。需要在 trait 上新增：

```rust
fn maturity_date(&self) -> NaiveDate;
```

若 `IterativeBootstrapper` 強制回報 `PillarAlignment::MaxDate`（不允許外部設定），`maturity_date()` 不會在 iterative bootstrapper 的路徑中被呼叫，所以加到 `Instrument` trait 對現有實作的影響最小。可以在各商品的 `maturity_date()` 實作中先回傳各自的合約到期日（IRS 的最後一個付款日，Deposit 的到期日），不影響現有 `max_date()` 語義。

---

## 主題二：PrecomputedDiscountCurve 實作細節

### 攻擊 `requiredDates` 方案的最弱假設

Ray 提出在 `SimpleInstrument` 與 `InterestRateIndex` 上加入 `requiredDates(horizon)` 方法，讓 calibrator 在 LM 開始前收集所有需要預計算的折現日期。

**這個方案中最弱、也是最危險的假設是：`requiredDates` 的實作可以永遠與 NPV 計算邏輯保持同步。**

這個假設是錯的，而且在長期維護中必然失效。

#### 論證：CompoundingRateIndex 的 lockout/lookback 日期映射

`CompoundingRateIndex` 計算每個 accrual day 的折現因子時，核心邏輯在 `accrual_to_fixing()` 中：每個業務日 d 映射到對應的 fixing date，受 lookback shift、lockout period、broken period 規則控制。這個映射是非平凡的——lookout/lockout 規則因商品不同而異，且可能在未來新增新的 convention。

`requiredDates(horizon)` 的實作必須**完整複製**這個映射邏輯，才能正確回傳「NPV 計算時實際會被 `discount()` 查詢的日期」。這製造了一個**雙重維護義務**：

```
accrual_to_fixing() 的任何變更 ↔ requiredDates() 同步更新
```

若未來有人為某個新的 fixing date convention 修改 `accrual_to_fixing()`，但忘記更新 `requiredDates()`，會發生以下情況：
- `requiredDates()` 回傳的日期集中**缺少新 convention 需要的日期**
- `PrecomputedDiscountCurve` 建構時沒有預計算這些日期
- NPV 計算時查詢到 cache miss → 可能回傳 fallback 值或 panic
- **這個錯誤不會在編譯期被發現**，也不會在簡單測試中被捕捉（只有在包含新 convention 的校準集中才會觸發）

這是靜默錯誤（silent incorrect output）的典型溫床。

#### 替代方案：Proxy 模式（DateCollectingDiscountCurve）

不需要在任何 instrument 或 index 上增加 `requiredDates` 方法。取而代之，在第一次 LM 迭代時使用一個「觀察型」DiscountCurve wrapper：

```rust
struct DateCollectingDiscountCurve {
    underlying: Arc<dyn DiscountCurve>,
    collected: Arc<Mutex<HashSet<NaiveDate>>>,
}

impl DiscountCurve for DateCollectingDiscountCurve {
    fn discount(&self, date: NaiveDate) -> f64 {
        self.collected.lock().unwrap().insert(date);
        self.underlying.discount(date)
    }
}
```

校準流程：

```rust
// 第一次迭代：使用 DateCollectingDiscountCurve
let collector = Arc::new(Mutex::new(HashSet::new()));
let collecting_curve = /* 包裝 trial_curve 的 discount curve */;
let first_iter_npvs = compute_npvs(instruments, &collecting_curve);
let required_dates: HashSet<NaiveDate> = Arc::try_unwrap(collector)
    .unwrap()
    .into_inner()
    .unwrap();

// 後續迭代：用 required_dates 建構 PrecomputedDiscountCurve
for iteration in 1.. {
    let trial_discount = trial_curve.to_discount_curve();
    let precomputed = PrecomputedDiscountCurve::new(
        trial_discount, &sorted_dates, CacheStrategy::Dense { ... }
    );
    let npvs = compute_npvs(instruments, &precomputed);
    // ...
}
```

**這個方案的關鍵優點**：required dates 的收集是 NPV 計算本身的副作用——**任何** `discount()` 呼叫都被記錄，不管來自哪個商品或哪個 index，不管 convention 如何變化，不需要任何額外的介面承諾。

**代價**：第一次 LM 迭代以全成本執行（無 precompute 加速）。若 LM 典型收斂需要 15–20 次迭代，第一次迭代只佔 5–7% 的計算成本，是可接受的代價。

#### 關於 wrapper 設計的 OnceLock 問題

Ray 的 wrapper 設計方向正確，但有兩個技術問題需要修正：

**問題一：命名不一致。** struct 名稱 `PrecomputedDiscountCurveWrapper` 與 impl 目標 `CurveWithPrecomputedDiscount` 不同，應統一。

**問題二：`to_discount_curve()` 在單次 LM 迭代中被呼叫 n 次（n = 商品數量）。** 若每次呼叫都重新建構 `PrecomputedDiscountCurve`（O(D) 次折現計算 + 排序），整個 precompute 機制反而增加了計算成本。解法是用 `OnceLock` 快取：

```rust
pub struct CurveWithPrecomputedDiscount {
    base_rate_curve: Arc<dyn InterestRateCurve>,
    required_dates:  Arc<[NaiveDate]>,           // 已排序
    cached_discount: OnceLock<Arc<dyn DiscountCurve>>,
}

impl InterestRateCurve for CurveWithPrecomputedDiscount {
    fn to_discount_curve(&self) -> Arc<dyn DiscountCurve> {
        Arc::clone(self.cached_discount.get_or_init(|| {
            let base = self.base_rate_curve.to_discount_curve();
            Arc::new(PrecomputedDiscountCurve::new(
                base,
                &self.required_dates,
                CacheStrategy::Auto,
            ))
        }))
    }
    fn to_zero_rate_curve(&self) -> Arc<dyn ZeroRateCurve> {
        self.base_rate_curve.to_zero_rate_curve()
    }
    fn to_inst_forward_curve(&self) -> Arc<dyn InstForwardCurve> {
        self.base_rate_curve.to_inst_forward_curve()
    }
    fn year_fraction_calculator(&self) -> &YearFractionCalculator {
        self.base_rate_curve.year_fraction_calculator()
    }
}
```

每次 LM 迭代建立一個新的 `CurveWithPrecomputedDiscount`（`OnceLock` 為空），第一次呼叫 `to_discount_curve()` 觸發建構並快取，後續 n-1 次呼叫只做 `Arc::clone`（~3ns）。

`required_dates` 使用 `Arc<[NaiveDate]>` 而非 `Arc<HashSet<NaiveDate>>`，因為 `PrecomputedDiscountCurve::new` 需要已排序的 slice，提前排序並以 Arc 共享可避免每次迭代重複排序。

---

## 立場匯整

| 議題 | Ray 提案 | 我的立場 | 相對 answer_3 的變化 |
|---|---|---|---|
| NPV/Market_PV01 正規化 | 採用 Gemini 建議 | 接受，但需加 PV01 > ε 保護 | **改變**：撤回「方形系統不需要正規化」 |
| PillarAlignment enum | 沿用 claude_answer_3 設計 | 接受；需在 `Instrument` trait 新增 `maturity_date()` | 不變 |
| `requiredDates` trait 方法 | 新增到 Instrument/Index | **反對**；改用 Proxy 模式 | 新立場 |
| CurveWithPrecomputedDiscount wrapper | 架構方向正確 | 接受，但需加 OnceLock + 命名統一 | 細化 |
