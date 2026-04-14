use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::instrument::{Position, SimpleInstrument};
use crate::market::market::Market;
use crate::time::period::Period;


pub trait SimpleInterestRateInstrumentGenerator {
    fn profit_and_loss_market(&self) -> &Arc<dyn Market>;

    fn generate_with_maturity_date(
        &self,
        position: Position,
        trade_date: NaiveDate,
        maturity_date: NaiveDate,
        start_date_opt: Option<NaiveDate>,
    ) -> Result<Arc<dyn SimpleInstrument>, String>;

    fn generate_with_maturity_tenor(
        &self,
        position: Position,
        trade_date: NaiveDate,
        maturity_tenor: Period,
        start_date_opt: Option<NaiveDate>,
    ) -> Result<Arc<dyn SimpleInstrument>, String>;

    /// 將市場原始報價轉換為 Bootstrapping 用的等效利率。
    ///
    /// 預設行為是直接回傳原值（適用於 Deposit、IRS 等）。
    /// 特殊的 Generator（如 SOFR Future）可以覆寫：
    /// ```ignore
    /// fn market_rate(&self, market_quote: f64) -> f64 {
    ///     0.01 * (100.0 - market_quote)
    /// }
    /// ```
    fn market_rate(&self, market_quote: f64) -> f64 {
        market_quote
    }
}
