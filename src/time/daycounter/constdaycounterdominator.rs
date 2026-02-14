use std::rc::Rc;

use serde::Deserialize;

use super::daycounter::{
    DayCounterNumerator, 
    DayCounterDominator,
    DayCounterDominatorGenerator,
    DayCounterGenerationError
};
use crate::time::schedule::schedule::Schedule;


pub struct ConstDayCounterDominator {
    dominator_value: f64
}

impl ConstDayCounterDominator {
    pub fn new(dominator_value: f64) -> ConstDayCounterDominator {
        ConstDayCounterDominator {dominator_value: dominator_value}
    }

    pub fn dominator_value(&self) -> f64 {
        self.dominator_value
    }
}

impl DayCounterDominator for ConstDayCounterDominator {
    #[inline]
    fn year_fraction(&self, 
                     start_date: chrono::NaiveDate, 
                     end_date: chrono::NaiveDate, 
                     numerator: &std::rc::Rc<dyn DayCounterNumerator>) -> f64 {
        numerator.days_between(start_date, end_date) / self.dominator_value
    }
}

#[derive(Deserialize)]
pub struct ConstDayCounterDominatorGenerator {
    dominator_value: f64
}


impl ConstDayCounterDominatorGenerator {
    pub fn new(dominator_value: f64) -> ConstDayCounterDominatorGenerator {
        ConstDayCounterDominatorGenerator{dominator_value: dominator_value}
    }

    pub fn dominator_value(&self) -> f64 {
        self.dominator_value
    }
}


impl DayCounterDominatorGenerator for ConstDayCounterDominatorGenerator {
    fn generate(&self, _schedule_opt: Option<&Schedule>) -> Result<Rc<dyn DayCounterDominator>, DayCounterGenerationError> {
        Ok(Rc::new(ConstDayCounterDominator::new(self.dominator_value)))
    }
}