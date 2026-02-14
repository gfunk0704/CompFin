use std::rc::Rc;

use chrono::{
    Datelike, 
    NaiveDate
};

use super::super::daycounter::{
    DayCounterNumerator,
    DayCounterNumeratorGenerator,
    DayCounterGenerationError
};
use super::super::super::utility::{
    is_leap, 
    leap_years_between
};
use super::super::super::schedule::schedule::Schedule;

pub struct NoLeapNumerator;

impl NoLeapNumerator {
    pub fn new() -> NoLeapNumerator {
        NoLeapNumerator {}
    }
}

impl DayCounterNumerator for NoLeapNumerator {
    fn days_between(&self, d1: NaiveDate, d2: NaiveDate) -> f64 {
        let mut days = (d2 - d1).num_days() as f64;
        let y1 = d1.year();
        let y2 = d2.year();
        if (y1 == y2) && is_leap(y1) {
            let leap_day = NaiveDate::from_ymd_opt(y1, 2, 29).unwrap();
            if (d1 < leap_day) && (d2 >= leap_day) {
                days -= 1.0;
            }
        } else {
            days -= leap_years_between(y1, y2) as f64;
            if is_leap(y1) {
                if d1 < NaiveDate::from_ymd_opt(y1, 2, 29).unwrap() {
                    days -= 1.0;
                }
            }

            if is_leap(y2) {
                if d1 >= NaiveDate::from_ymd_opt(y2, 2, 29).unwrap() {
                    days -= 1.0;
                }
            }
        }
        days
    }
}


pub struct NoLeapNumeratorGenerator;

impl NoLeapNumeratorGenerator {
    pub fn new() -> NoLeapNumeratorGenerator {
        NoLeapNumeratorGenerator {}
    }
}

impl DayCounterNumeratorGenerator for NoLeapNumeratorGenerator {
    fn generate(&self, _schedule_opt: Option<&Schedule>) -> Result<Rc<dyn DayCounterNumerator>, DayCounterGenerationError> {
        Ok(Rc::new(NoLeapNumerator::new()))
    }
}

