# COMPFIN.md

本文件為 **CompFin** 專案的統一開發指引，供 Claude Code 與 Gemini CLI 共同參照。

---

## 專案概述

CompFin 是一個以 Rust 撰寫的企業級量化金融定價引擎，架構對標 Murex，涵蓋利率衍生性商品的曲線建構、模型校準與結構型商品評價。Python 版本作為工作環境中的參考實作與驗證基準。

### 領域範圍

- **核心產品線:** 利率衍生性商品（IRS、Deposit、Swaption）、Yield Curve 建構（Iterative / LeastSquare Bootstrapping）
- **定價模型:** Bachelier / Black Swaption、Hull-White 單因子模型（含 Turfus 記號之條件折現曲線框架）
- **結構型商品（規劃中）:** CMS Steepener、Inverse Floater、Range Accrual、TARN 變體、Quanto、Auto-call、Bermudan Swaption
- **合規需求:** FRTB / CRR3 風險敏感度監控，資料結構需預留 Sensitivities 欄位
- **未來擴充:** FX 市場（`FxMarket` trait 已預設但尚未實作，請勿破壞該抽象邊界）

### 參考系統與文獻

- **系統:** Murex（主要架構對標）、QuantLib（數值驗證）、Numerix、Algo、Bloomberg、FIS
- **文獻:** Turfus（條件折現曲線）、Henrard、Caspers/QL GSR 慣例、Clark (2012) 市場慣例

---

## 常用指令

```bash
cargo build              # Debug 建置
cargo build --release    # Release 建置
cargo check              # 快速語法/借用檢查（開發期間優先使用）
cargo test               # 執行所有測試
cargo test <test_name>   # 執行單一測試
cargo clippy             # Lint 檢查
cargo fmt                # 格式化
cargo doc --open         # 生成並開啟文件
```

---

## 核心架構

### 兩階段生命週期

| 階段 | 型別 | 說明 |
|---|---|---|
| **載入階段** | `ManagerBuilder<V>` | 從 JSON 反序列化，單執行緒可變，依固定順序載入物件 |
| **執行階段** | `FrozenManager<V>` | `build()` 後轉為不可變 `Arc<HashMap>` 快照，多執行緒無鎖存取 |

所有儲存的 trait object 必須為 `Send + Sync`。

### 模組架構

| 模組 | 職責 |
|---|---|
| `configuration.rs` | 系統初始化編排，依相依順序載入所有 manager |
| `instrument/` | `Deposit`、`InterestRateSwap`、legs、cashflow 生成器 |
| `interestrate/` | 利率指標（`TermRate`、`CompoundingRate`）、快取後端 |
| `market/` | `SingleCurrencyMarket`、結算條件、折現曲線連結 |
| `model/interestrate/` | Bootstrapping、分段多項式曲線建構、`argmin` 校準 |
| `math/` | `Curve` trait 體系、Lagrange 內插、根求解器 |
| `pricer/` | 泛型 `Pricer<S, T>` trait，計算 MTM 與 P&L |
| `value/` | `CashFlows`、NPV 計算（含幣別/結算日驗證） |
| `time/` | 日曆、排程、天數計算慣例 |
| `manager/` | `ManagerBuilder<V>` / `FrozenManager<V>` 泛型基礎設施 |

### 關鍵 Trait

- **`Instrument` / `InstrumentWithLinearFlows`** — 現金流行為
- **`InterestRateIndex`** — 利率查詢與投影
- **`InterestRateCurve`** — 折現因子、零利率、遠期利率；子 trait: `DiscountCurve`、`ZeroRateCurve`、`InstForwardCurve`
- **`LegCharacters`** — 固定/浮動 leg 參數（使用 `RwLock` 實現執行期可調整）
- **`HolidayCalendar`** — 工作日邏輯
- **`Pricer<S, T>`** — 泛型於商品型別 `S` 與市場資料型別 `T`
- **`JsonLoader`** — 可插拔的反序列化搭配依賴注入；使用 `Named<T>` 搭配 `#[serde(flatten)]`

### 曲線建構與校準

- `IterativeBootstrapper`：逐 pillar 根求解，搭配分段凍結加速策略（已解 pillar 凍結後再校準下一段）
- `FlatForwardCurve`：單 pillar 場景
- `BootstrappingTrait`：跨內插目標（`LogDiscount`、`ZeroRate`、`InstForward`）提供初始值/區間
- `FreezableInstrument`：凍結已解前綴的 NPV，僅計算尾部流量（對 `CompoundingRateIndex` 高每流成本場景有顯著效益）
- `PrecomputedDiscountCurve`：校準期間快取折現因子查詢
- `LeastSquareCurveCalibration`（規劃中）：trust-region 方法，對應 QuantLib GlobalBootstrapper

### JSON 設定載入順序

依相依性順序載入：

1. `holiday_calendar[]` — 假日規則（FixedDate、NthWeekday、LastWeekday、Easter 變體）
2. `day_count[]` — 天數計算慣例
3. `schedule[]` — 期間生成器與 stub 規則
4. `market[]` — 幣別、折現曲線、結算條件
5. `interest_rate_index[]` — 利率指標與複利慣例
6. `deposit_generator[]` / `swap_generator[]` — 商品範本

---

## 程式碼撰寫準則

### 錯誤處理

- **絕對禁止**在非測試程式中使用 `unwrap()` 或 `expect()`
- 使用 `thiserror` 定義所有自訂錯誤類型，透過 `#[from]` 組合跨模組錯誤
- 錯誤類型就近定義於產生它的模組（如 `ManagerError`、`CurveGenerationError`）

### 數值計算

- 利率與年分數使用 `f64`；需高精度時使用 `rust_decimal`（Banker's rounding）
- 浮點數比較**一律**使用容差值，禁止 `==`
- 進位策略由 `PricingCondition` 參數化（`deterministic_flow`、`estimated_index`、`estimated_flow` 為獨立開關）

### 並發安全

- 共享狀態透過 `Arc` 傳遞
- 內部可變性使用 `RwLock`（非 `RefCell`），因所有型別必須 `Send + Sync`
- `Arc::clone` 成本極低（~3–5 ns），不值得為避免它而引入生命週期參數複雜度

### 慣用風格

- 偏好 `unwrap_or_else` 而非 `if let` 處理 `Option`
- 偏好 `crate::` 而非 `super::` imports
- 區間慣例：右閉 `(t_{i-1}, t_i]`，全專案一致
- `market_rate()` 邏輯歸屬 `SimpleInstrumentGenerator`（報價慣例轉換如 `0.01 * (100 - quote)` 是 generator 的職責）

### 相依性管理

**⚠️ 嚴格限制：** 未經明確同意，禁止引入新的外部 crate。

現有核心相依：

| 用途 | Crate |
|---|---|
| 日期時間 | `chrono` |
| 線性代數 | `nalgebra` |
| 序列化 | `serde`、`serde_json` |
| 錯誤處理 | `thiserror` |
| 唯一識別 | `uuid` |
| 高精度數值 | `rust_decimal` |
| 最佳化求解 | `argmin`、`argmin-math` |

---

## 已確立的設計原則

以下為過往討論中確立的原則，請勿違背：

1. **慣例分歧是預期行為，不是 bug：** Murex/Henrard、QuantLib/Caspers、Turfus 各自中心化狀態變數的方式不同。跨系統對齊不是目標，**內部一致性**才是。
2. **狀態變數中心化影響結果：** `x(t) = r(t) - f(0,t)`（Caspers/QL）vs. Turfus 中心化，導致 `x=0 ≠ y=0`，這是 swaption 定價系統性偏差的根本原因。
3. **正確性優先於最佳化：** `PrecomputedDiscountCurve` 與 `CachedInterestRateIndex` 延後至透過 iterative bootstrapping 對齊 Python 參考實作後再啟用。
4. **Rust stable 限制：** `Arc<dyn SimpleInstrument>` 無法向上轉型為 `Arc<dyn BootstrappableInstrument>`，supertrait 方案在 stable Rust 不可行。
5. **數學推導獨立驗證：** 先自行推導，再請求驗證；追查根因而非直接接受修正。

---

## 開發工作流程

本專案採用**多 LLM 協作模式**，各角色分工如下：

### 角色分配

| 角色 | 負責 LLM | 職責 |
|---|---|---|
| 設計討論 | Claude Sonnet + Gemini Pro | 回應架構與設計問題，交叉評論 |
| 程式生成 | Claude Opus / Claude Code | 根據討論結論生成實作程式碼 |
| Code Review | Gemini Pro / Gemini CLI | 審查生成的程式碼 |
| 最終決策 | **Ray（人類）** | 評估分歧、決定執行方案 |

### 迭代流程

```
1. Ray 提出今日議題
2. Claude Sonnet 與 Gemini Pro 各自回覆
3. Claude Sonnet 與 Gemini Pro 交叉評論對方的回覆
4. 重複步驟 2-3，直到累積足夠修改
5. Ray 決定執行方法
6. Claude Opus / Claude Code 根據討論生成程式碼
7. Gemini Pro / Gemini CLI 做 code review
8. Ray 決定最終版本
9. 回到步驟 1
```

### 協作注意事項

- Claude 的分析在過往討論中多次發現 Gemini 提案的錯誤（double-discounting 誤診、TypeId-based PricerMap 不可行、supertrait 向上轉型限制），但最終判斷權在 Ray
- 遇到領域知識模糊之處（結算慣例、stub 規則、日曆位移邏輯），**主動提問**，絕對不要自行捏造市場規則
- Python 參考實作為正確性基準；Murex 行為為會計慣例標準

---

## 當前開發狀態

### 進行中

- `IterativeBootstrapper` 已實作（pillar-by-pillar 根求解 + `FlatForwardCurve` + `BootstrappingTrait`）
- `FreezableInstrument` 部分實作（凍結前綴 NPV 核心設計完成，`iterativebootstrapper.rs` 實作中斷）
- `InterestRateCurveCalibrationHelper` 的 `market_rate` 尚未接線（目前使用硬編碼佔位值）；`into_instrument()` 待新增
- Hull-White 條件折現曲線框架已在 Python 實作；數個 bug 已協作修正

### 待辦

- 完成 `FreezableInstrument` 與凍結感知的 bootstrapping（`solve_subsequent_pillar_with_freeze`）
- 將 `market_rate` 從 `InterestRateQuoteSheet` 經 `InterestRateCurveCalibrationHelper` 接入 `IterativeBootstrapper`
- `LeastSquareCurveCalibration`（trust-region，QuantLib GlobalBootstrapper 對應物）
- Hull-White 結構型商品定價（Bermudan callables for CMS Steepener、Inverse Floater、Range Accrual、TARN）