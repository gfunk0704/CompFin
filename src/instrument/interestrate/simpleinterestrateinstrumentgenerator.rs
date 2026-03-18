use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::instrument::{
    Position, 
    SimpleInstrument
};
use crate::market::market::Market;
use crate::time::period::Period;


pub trait SimpleInterestRateInstrumentGenerator {
    fn profit_and_loss_market(&self) -> &Arc<dyn Market>;

    fn generate_with_maturity_date(
        &self, 
        position: Position, 
        trade_date: NaiveDate,
        maturity_date: NaiveDate,
        start_date: Option<NaiveDate>
    ) -> Arc<dyn SimpleInstrument>;

    fn generate_with_maturity_tenor(
        &self, 
        position: Position, 
        trade_date: NaiveDate,
        maturity_tenor: Period,
        start_date: Option<NaiveDate>
    ) -> Arc<dyn SimpleInstrument> {
        let maturity_date = trade_date + maturity_tenor;
        self.generate_with_maturity_date(position, trade_date, maturity_date, start_date)
    }
}