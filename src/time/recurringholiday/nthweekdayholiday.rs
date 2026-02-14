use std::collections::HashSet;

use chrono::{
    NaiveDate, 
    Weekday
};

use super::recurringholiday::RecurringHoliday;

#[derive(Clone)]
pub struct NthWeekdayHoliday {
    month: u32,
    n: u8,
    weekday: Weekday,
}

impl NthWeekdayHoliday {
    pub fn new(month: u32, n: u8, weekday: Weekday) -> Option<NthWeekdayHoliday> {
        if !(1..=12).contains(&month) || !(1..=5).contains(&n) {
            None
        } else {
            Some(NthWeekdayHoliday { month, n, weekday })
        }
    }

    pub fn month(&self) -> u32 {
        self.month
    }

    pub fn n(&self) -> u8 {
        self.n
    }

    pub fn weekday(&self) -> Weekday {
        self.weekday
    }
}

impl RecurringHoliday for NthWeekdayHoliday {
    fn get_holiday(&self, year: i32) -> HashSet<NaiveDate> {
        let mut holiday_set = HashSet::new();
        
        if let Some(date) = NaiveDate::from_weekday_of_month_opt(
            year, 
            self.month, 
            self.weekday, 
            self.n
        ) {
            holiday_set.insert(date);
        }
        
        holiday_set
    }
}
