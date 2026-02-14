use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};


pub trait RecurringHoliday {
    
    fn get_holiday(&self, year: i32) -> HashSet<NaiveDate>;

    fn is_holiday(&self, d: &NaiveDate) -> bool {
        let holiday_set = self.get_holiday(d.year());
        holiday_set.contains(d)
    }
}