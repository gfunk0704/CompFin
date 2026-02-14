use std::collections::HashSet;

use chrono::{Datelike, Days, NaiveDate, Weekday};

use super::recurringholiday::RecurringHoliday;

#[derive(Clone)]
pub struct LastWeekdayHoliday {
    month: u32,
    weekday: Weekday,
}

impl LastWeekdayHoliday {
    pub fn new(month: u32, weekday: Weekday) -> Option<LastWeekdayHoliday> {
        if !(1..=12).contains(&month) {
            None
        } else {
            Some(LastWeekdayHoliday { month, weekday })
        }
    }

    pub fn month(&self) -> u32 {
        self.month
    }

    pub fn weekday(&self) -> Weekday {
        self.weekday
    }
}

impl RecurringHoliday for LastWeekdayHoliday {
    fn get_holiday(&self, year: i32) -> HashSet<NaiveDate> {
        // Get the first day of the next month
        let first_of_next_month = if self.month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(year, self.month + 1, 1).unwrap()
        };
        
        // Calculate days to go back to reach the target weekday
        let current_weekday = first_of_next_month.weekday();
        let target_weekday = self.weekday;
        
        // Calculate the difference in days (going backwards)
        let days_back = ((current_weekday.num_days_from_monday() as i32 
                         - target_weekday.num_days_from_monday() as i32 + 7) % 7) as u64;
        
        // If it's the same day, go back a full week
        let days_back = if days_back == 0 { 7 } else { days_back };
        
        let mut holiday_set = HashSet::new();
        holiday_set.insert(first_of_next_month - Days::new(days_back));
        holiday_set
    }
}
