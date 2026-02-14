
use std::collections::{HashMap, HashSet};

use chrono::{
    Datelike, 
    NaiveDate, 
    Weekday
};

use super::recurringholiday::RecurringHoliday;
use super::weekendadjustment::{
    WeekendAdjustment, 
    WeekendAdjustmentRule
};

#[derive(PartialEq, Eq, Clone, Copy)]
enum WeekendAdjustmentShiftCheck {
    MayShiftToPreviousYear = 1,
    MayShiftToNextYear = -1,
    None 
}

#[derive(Clone)]
pub struct FixedDateHoliday {
    month: u32,
    day: u32,
    weekend_adjustment_rules: WeekendAdjustmentRule,
    shift_check: WeekendAdjustmentShiftCheck
}

impl FixedDateHoliday {
    pub fn new(month: u32, day: u32, weekend_adjustment_map: &HashMap<Weekday, WeekendAdjustment>) -> Option<FixedDateHoliday> {
        let n_weekend = weekend_adjustment_map.len() as u32;
        let shift_check = if n_weekend > 0 {
            if month == 1 && day <= n_weekend {
                WeekendAdjustmentShiftCheck::MayShiftToPreviousYear
            } else if month == 12 && day > 31 - n_weekend {
                WeekendAdjustmentShiftCheck::MayShiftToNextYear
            } else {
                WeekendAdjustmentShiftCheck::None
            }
        } else {
            WeekendAdjustmentShiftCheck::None
        };

        Some(FixedDateHoliday { 
            month, 
            day,
            weekend_adjustment_rules: WeekendAdjustmentRule::new(weekend_adjustment_map),
            shift_check
        })
    }

    pub fn month(&self) -> u32 {
        self.month
    }

    pub fn day(&self) -> u32 {
        self.day
    }

    pub fn weekend_adjustment_rules(&self) -> &WeekendAdjustmentRule {
        &self.weekend_adjustment_rules
    }

    fn get_holiday_impl(&self, year: i32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(year, self.month, self.day)
            .map(|d| self.weekend_adjustment_rules.adjust(d))
    }
}

impl RecurringHoliday for FixedDateHoliday {
    fn get_holiday(&self, year: i32) -> HashSet<NaiveDate> {
        let mut holiday_set = HashSet::new();
       
        // Check the primary year
        if let Some(d1) = self.get_holiday_impl(year) {
            if d1.year() == year {
                holiday_set.insert(d1);
            }
        }

        // Check adjacent year if weekend adjustment might shift the date
        if self.shift_check != WeekendAdjustmentShiftCheck::None {
            if let Some(d2) = self.get_holiday_impl(year + self.shift_check as i32) {
                if d2.year() == year {
                    holiday_set.insert(d2);  // BUG FIX: This line was missing
                }
            }
        }

        holiday_set
    }
}
