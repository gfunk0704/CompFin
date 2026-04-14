use std::collections::HashMap;
use std::sync::Arc;

use crate::marketdata::interestrate::interestratequotesheet::InterestRateQuoteSheet;
use crate::model::interestrate::interestratecurve::InterestRateCurve;


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateCurveMarketData
// ─────────────────────────────────────────────────────────────────────────────
//
// 利率曲線相關的市場資料：
//   quote_book — 各商品的市場報價，key 為 generator 名稱
//   curves     — 已 calibrate 的利率曲線，key 為曲線名稱

pub struct InterestRateCurveMarketData {
    quote_book: HashMap<String, InterestRateQuoteSheet>,
    curves:     HashMap<String, Arc<dyn InterestRateCurve>>,
}

impl InterestRateCurveMarketData {
    pub fn new() -> Self {
        Self {
            quote_book: HashMap::new(),
            curves:     HashMap::new(),
        }
    }

    // ── Quote book ────────────────────────────────────────────────────────────

    pub fn add_quote_sheet(&mut self, name: impl Into<String>, sheet: InterestRateQuoteSheet) {
        self.quote_book.insert(name.into(), sheet);
    }

    pub fn get_quote_sheet(&self, name: &str) -> Option<&InterestRateQuoteSheet> {
        self.quote_book.get(name)
    }

    pub fn quote_book(&self) -> &HashMap<String, InterestRateQuoteSheet> {
        &self.quote_book
    }

    // ── Curves ────────────────────────────────────────────────────────────────

    pub fn insert_curve(
        &mut self,
        name:  impl Into<String>,
        curve: Arc<dyn InterestRateCurve>,
    ) {
        self.curves.insert(name.into(), curve);
    }

    pub fn get_curve(&self, name: &str) -> Option<&Arc<dyn InterestRateCurve>> {
        self.curves.get(name)
    }

    pub fn curves(&self) -> &HashMap<String, Arc<dyn InterestRateCurve>> {
        &self.curves
    }
}

impl Default for InterestRateCurveMarketData {
    fn default() -> Self {
        Self::new()
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateMarketData
// ─────────────────────────────────────────────────────────────────────────────
//
// 利率商品所需的完整市場資料集合。
// 目前只有 curve_market_data，未來可加入 volatility surface、fixing history 等。

pub struct InterestRateMarketData {
    curve_market_data: InterestRateCurveMarketData,
}

impl InterestRateMarketData {
    pub fn new() -> Self {
        Self {
            curve_market_data: InterestRateCurveMarketData::new(),
        }
    }

    pub fn curve_market_data(&self) -> &InterestRateCurveMarketData {
        &self.curve_market_data
    }

    pub fn curve_market_data_mut(&mut self) -> &mut InterestRateCurveMarketData {
        &mut self.curve_market_data
    }
}

impl Default for InterestRateMarketData {
    fn default() -> Self {
        Self::new()
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// MarketDataSet
// ─────────────────────────────────────────────────────────────────────────────
//
// 系統中所有市場資料的頂層容器。
// 對應 Configuration 的靜態設定（generators、calendars 等），
// MarketDataSet 持有動態的市場資料（quotes、calibrated curves）。
//
// 未來可擴充加入：
//   fx:     FxMarketData
//   credit: CreditMarketData

pub struct MarketDataSet {
    interest_rate: InterestRateMarketData,
}

impl MarketDataSet {
    pub fn new() -> Self {
        Self {
            interest_rate: InterestRateMarketData::new(),
        }
    }

    // ── Interest rate ─────────────────────────────────────────────────────────

    pub fn interest_rate(&self) -> &InterestRateMarketData {
        &self.interest_rate
    }

    pub fn interest_rate_mut(&mut self) -> &mut InterestRateMarketData {
        &mut self.interest_rate
    }

    // ── 常用的便利方法，避免呼叫端一直往下鑽 ─────────────────────────────────

    /// 取得 quote sheet。
    pub fn get_quote_sheet(&self, name: &str) -> Option<&InterestRateQuoteSheet> {
        self.interest_rate.curve_market_data().get_quote_sheet(name)
    }

    /// 新增或更新 quote sheet。
    pub fn add_quote_sheet(&mut self, name: impl Into<String>, sheet: InterestRateQuoteSheet) {
        self.interest_rate
            .curve_market_data_mut()
            .add_quote_sheet(name, sheet);
    }

    /// 取得已 calibrate 的 curve。
    pub fn get_curve(&self, name: &str) -> Option<&Arc<dyn InterestRateCurve>> {
        self.interest_rate.curve_market_data().get_curve(name)
    }

    /// 新增或更新 curve（calibration 完成後呼叫）。
    pub fn insert_curve(
        &mut self,
        name:  impl Into<String>,
        curve: Arc<dyn InterestRateCurve>,
    ) {
        self.interest_rate
            .curve_market_data_mut()
            .insert_curve(name, curve);
    }

    /// 取得完整的 quote book（供 calibrator 使用）。
    pub fn quote_book(&self) -> &HashMap<String, InterestRateQuoteSheet> {
        self.interest_rate.curve_market_data().quote_book()
    }
}

impl Default for MarketDataSet {
    fn default() -> Self {
        Self::new()
    }
}
