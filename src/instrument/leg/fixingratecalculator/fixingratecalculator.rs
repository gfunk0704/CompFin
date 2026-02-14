use std::collections::HashSet;
use std::rc::Rc;

use chrono::NaiveDate;

use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::schedule::schedule::Schedule;


pub enum FixingRateType {
    TermRate,
    DailyCompounding
}

pub trait FixingRateCalculator {
    fn index(&self) -> &Rc<dyn InterestRateIndex>;

    fn fixing_rate_type(&self) -> FixingRateType;

    fn relative_dates(&self,
                      i: usize) -> HashSet<NaiveDate>;

    fn fixing(&self,
              i: usize,
              forward_curve: &Rc<dyn InterestRateCurve>,
              pricing_condition: &PricingCondition) -> f64;
}


pub trait  FixingRateCalculatorGenerator {
    fn index(&self) -> &Rc<dyn InterestRateIndex>;

    fn fixing_rate_type(&self) -> FixingRateType;

    fn generate(&self,
                schedule: &Schedule) -> Rc<dyn FixingRateCalculator>;
}







