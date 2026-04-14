use crate::pricingcondition::PricingCondition;
use crate::value::npv::NPV;


pub trait Pricer<S: ?Sized, T> {
    fn market_value(
        &self,
        instrument: &S,
        market_data: &T,
        pricing_condition: &PricingCondition
    ) -> Option<NPV>;

    fn econ_profit_and_loss(
        &self,
        instrument: &S,
        market_data: &T,
        pricing_condition: &PricingCondition
    ) -> Option<NPV>;
}