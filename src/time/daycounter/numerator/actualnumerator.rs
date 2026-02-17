use std::sync::Arc; // 變更：Rc → Arc

use chrono::NaiveDate;

use super::super::daycounter::{
    DayCounterNumerator,
    DayCounterNumeratorGenerator,
    DayCounterGenerationError
};
use super::super::super::schedule::schedule::Schedule;


pub struct ActualNumerator;

impl ActualNumerator {
    pub fn new() -> ActualNumerator {
        ActualNumerator {}
    }
}

impl DayCounterNumerator for ActualNumerator {
    #[inline]
    fn days_between(&self, d1: NaiveDate, d2: NaiveDate) -> f64 {
        (d2 - d1).num_days() as f64
    }
}

pub struct ActualNumeratorGenerator;

impl ActualNumeratorGenerator {
    pub fn new() -> ActualNumeratorGenerator {
        ActualNumeratorGenerator {}
    }
}

impl DayCounterNumeratorGenerator for ActualNumeratorGenerator {
    fn generate(
        &self,
        _schedule_opt: Option<&Schedule>,
    ) -> Result<Arc<dyn DayCounterNumerator>, DayCounterGenerationError> { // 變更：Rc → Arc
        Ok(Arc::new(ActualNumerator::new()))
    }
}
