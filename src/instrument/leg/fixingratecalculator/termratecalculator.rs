use std::collections::HashSet;
use std::rc::Rc;

use chrono::NaiveDate;

use crate::instrument::leg::fixingratecalculator::fixingratecalculator::{
    FixingRateCalculator, 
    FixingRateCalculatorGenerator, 
    FixingRateType
};
use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::schedule::schedule::Schedule;


pub struct TermRateCalculator {
    index: Rc<dyn InterestRateIndex>,
    fixing_dates: Vec<NaiveDate>
}

impl TermRateCalculator {
    pub fn new(index: Rc<dyn InterestRateIndex>,
               schedule: &Schedule) -> TermRateCalculator {
        let fixing_dates: Vec<NaiveDate> = schedule.
            schedule_periods().
            iter().
            map(|schedule_period| schedule_period.fixing_date()).
            collect();

        TermRateCalculator {
            index: index,
            fixing_dates: fixing_dates
        }
    }

    pub fn fixing_dates(&self) -> &Vec<NaiveDate> {
        &self.fixing_dates
    }
}

impl FixingRateCalculator for TermRateCalculator {
    fn index(&self) -> &Rc<dyn InterestRateIndex> {
        &self.index
    }

    fn fixing_rate_type(&self) -> FixingRateType {
        FixingRateType::DailyCompounding
    }

    fn relative_dates(&self,
                      i: usize) -> HashSet<NaiveDate> {
        self.index.relative_dates(self.fixing_dates[i])
    }

    fn fixing(&self,
              i: usize,
              forward_curve: &Rc<dyn InterestRateCurve>,
              pricing_condition: &PricingCondition) -> f64 {
        self.index.fixing_rate(self.fixing_dates[i], forward_curve, pricing_condition).unwrap()
    }
}


pub struct TermRateCalculatorGenerator {
    index: Rc<dyn InterestRateIndex>
}


impl FixingRateCalculatorGenerator for TermRateCalculatorGenerator {
    fn index(&self) -> &Rc<dyn InterestRateIndex> {
        &self.index
    }

    fn fixing_rate_type(&self) -> FixingRateType {
        FixingRateType::DailyCompounding
    }
    
    fn generate(&self,
                schedule: &Schedule) -> Rc<dyn FixingRateCalculator> {
        Rc::new(TermRateCalculator::new(self.index.clone(), schedule))
    }
}
