use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::time::period::Period;
use crate::value::cashflows::CashFlows;

use super::super::market::market::Market;
use super::super::pricingcondition::PricingCondition;

#[derive(Debug, Clone, Copy)]
pub enum Position {
    Buy = 1,
    Sell = -1
}


#[derive(Debug, Clone, Copy)]
pub enum CurveFunction {
    PayDiscount,
    PayForward,
    ReceiveDiscount,
    ReceiveForward
}


pub trait Instrument {
    fn max_date(&self) -> NaiveDate;
    
    fn position(&self) -> Position;

    fn profit_and_loss_market(&self) -> &Arc<dyn Market>;

    fn curve_name_map(&self) -> &HashMap<CurveFunction, String>;

    fn is_linear(&self) -> bool;
}


pub trait InstrumentWithLinearFlows {
    fn past_pay_flows(&self, pricing_condition: PricingCondition) -> CashFlows;

    fn past_receive_flows(&self, pricing_condition: PricingCondition) -> CashFlows;

    fn projected_pay_flows(&self, forward_curve_opt: Option<Arc<dyn InterestRateCurve>>, pricing_condition: PricingCondition) -> CashFlows;

    fn projected_receive_flows(&self, forward_curve_opt: Option<Arc<dyn InterestRateCurve>>, pricing_condition: PricingCondition) -> CashFlows;
}


pub trait SimpleInstrument: Instrument + InstrumentWithLinearFlows {
}


pub trait SimpleInstrumentGenerator {
    fn generate_with_maturity_tenor(&self, 
                                    position: Position,
                                    trade_date: NaiveDate,
                                    maturity_tenor: Period,
                                    effective_date_opt: Option<NaiveDate>) -> Arc<dyn SimpleInstrument>;

    fn generate_with_maturity_date(&self,
                                   position: Position,   
                                   trade_date: NaiveDate,
                                   maturity_date: NaiveDate,
                                   effective_date_opt: Option<NaiveDate>) -> Arc<dyn SimpleInstrument>;
}
