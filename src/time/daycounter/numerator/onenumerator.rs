use std::sync::Arc; // 變更：Rc → Arc

use chrono::NaiveDate;

use super::super::daycounter::{
    DayCounterNumerator,
    DayCounterNumeratorGenerator,
    DayCounterGenerationError
};
use super::super::super::schedule::schedule::Schedule;


pub struct OneNumerator;

impl OneNumerator {
    pub fn new() -> OneNumerator {
        OneNumerator {}
    }
}

impl DayCounterNumerator for OneNumerator {
    #[inline]
    fn days_between(&self, _d1: NaiveDate, _d2: NaiveDate) -> f64 {
        1.0
    }
}

pub struct OneNumeratorGenerator;

impl OneNumeratorGenerator {
    pub fn new() -> OneNumeratorGenerator {
        OneNumeratorGenerator {}
    }
}

impl DayCounterNumeratorGenerator for OneNumeratorGenerator {
    fn generate(
        &self,
        _schedule_opt: Option<&Schedule>,
    ) -> Result<Arc<dyn DayCounterNumerator>, DayCounterGenerationError> { // 變更：Rc → Arc
        Ok(Arc::new(OneNumerator::new()))
    }
}
