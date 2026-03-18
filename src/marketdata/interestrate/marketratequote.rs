use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::instrument::SimpleInstrument;
use crate::marketdata::quote::Quote;
use crate::time::period::Period;


pub trait QuoteWithInterestRateInstrument {
    fn generate_instrument(&self, trade_date: NaiveDate) -> Arc<dyn SimpleInstrument>;

    fn tenor(&self) -> Option<Period>;

    fn maturity_date(&self, trade_date: NaiveDate) -> NaiveDate;

    fn max_date(&self, trade_date: NaiveDate) -> NaiveDate;

    fn market_quote(&self) -> NaiveDate;
}


pub trait MarketRateQuote: Quote + QuoteWithInterestRateInstrument {

}