use std::rc::Rc;

use chrono::{Datelike, NaiveDate};

use super::daycounter::{
    DayCounterNumerator, 
    DayCounterDominator,
    DayCounterDominatorGenerator,
    DayCounterGenerationError
};
use super::super::schedule::schedule::Schedule;
use super::super::utility::is_leap;


pub struct ISDAActualDayCounterDominator {
}

impl ISDAActualDayCounterDominator {
    pub fn new() -> ISDAActualDayCounterDominator {
        ISDAActualDayCounterDominator {}
    }
}

fn get_dominator (year: i32) -> f64 {
    if is_leap(year) {
        366.0
    } else {
        365.0
    }
}

impl DayCounterDominator for ISDAActualDayCounterDominator {
    fn year_fraction(&self, 
                     start_date: chrono::NaiveDate, 
                     end_date: chrono::NaiveDate, 
                     numerator: &std::rc::Rc<dyn DayCounterNumerator>) -> f64 {
        let start_year = start_date.year();
        let end_year = end_date.year();
        if start_year == end_year {
            numerator.days_between(start_date, end_date) / get_dominator(start_year)
        } else {
            let mut d2 = NaiveDate::from_ymd_opt(start_year - 1, 12, 31).unwrap();
            let mut result = numerator.days_between(start_date, d2) / get_dominator(start_year);
            let mut d1 = NaiveDate::from_ymd_opt(end_year - 1, 12, 31).unwrap();
            result += numerator.days_between(d1, end_date) / get_dominator(end_year);
            for y in (start_year + 1)..end_year {
                d1 = NaiveDate::from_ymd_opt(y - 1, 12, 31).unwrap();
                d2 = NaiveDate::from_ymd_opt(y, 12, 31).unwrap();
                result += numerator.days_between(d1, d2) / get_dominator(y);
            }
            result
        }
    }
}



pub struct ISDAActualDayCounterDominatorGenerator;


impl ISDAActualDayCounterDominatorGenerator {
    pub fn new() -> ISDAActualDayCounterDominatorGenerator {
        ISDAActualDayCounterDominatorGenerator {}
    }
}


impl DayCounterDominatorGenerator for ISDAActualDayCounterDominatorGenerator {
    fn generate(&self, _schedule_opt: Option<&Schedule>) -> Result<Rc<dyn DayCounterDominator>, DayCounterGenerationError> {
        Ok(Rc::new(ISDAActualDayCounterDominator::new()))
    }
}