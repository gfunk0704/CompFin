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
}
