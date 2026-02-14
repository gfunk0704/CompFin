use std::collections::HashMap;
use std::rc::Rc;

use chrono::NaiveDate;

use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::value::cashflows::CashFlows;

use super::super::market::market::Market;
use super::super::pricingcondition::PricingCondition;
use super::super::value::npv::NPV;

#[derive(Debug, Clone, Copy)]
pub enum Position {
    Buy,
    Sell
}


#[derive(Debug, Clone, Copy)]
pub enum CurveFunction {
    PayDiscount,
    PayForward,
    ReceiveDiscount,
    ReceiveForward
}


pub trait Instrument {
    fn is_nonlinear(&self) -> bool;

    fn max_date(&self) -> NaiveDate;
    
    fn position(&self) -> Position;

    fn profit_and_loss_market(&self) -> Rc<dyn Market>;

    fn curve_name_map(&self) -> &HashMap<CurveFunction, String>;
}


pub trait SimpleInstrument: Instrument {
    fn past_pay_flows(&self, pricing_condition: PricingCondition) -> CashFlows;

    fn past_receive_flows(&self, pricing_condition: PricingCondition) -> CashFlows;

    fn projected_pay_flows(&self, forward_curve_opt: Option<Rc<dyn InterestRateCurve>>, pricing_condition: PricingCondition) -> CashFlows;

    fn projected_receive_flows(&self, forward_curve_opt: Option<Rc<dyn InterestRateCurve>>, pricing_condition: PricingCondition) -> CashFlows;
}