use std::collections::HashSet;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::super::period::Period;
use super::recurringholiday::RecurringHoliday;

#[derive(PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum EasterType {
    Western,
    Orthodox 
}

#[derive(Clone)]
pub struct EasterRelatedHoliday {
    easter_type: EasterType,
    shift_period: Period
}

impl EasterRelatedHoliday {
    pub fn new(easter_type: EasterType, shift_days: i32) -> Option<EasterRelatedHoliday> {
        Some(EasterRelatedHoliday { 
            easter_type, 
            shift_period: Period::days(shift_days)
        })
    }

    pub fn easter_type(&self) -> EasterType {
        self.easter_type
    }

    pub fn shift_period(&self) -> Period {
        self.shift_period
    }

    fn get_easter_day(&self, year: i32) -> Option<NaiveDate> {
        // Valid range for Easter calculation
        if !(1583..=4099).contains(&year) {
            return None;
        }

        let g = year % 19;
        
        let p = match self.easter_type {
            EasterType::Orthodox => {
                let i = (19 * g + 15) % 30;
                let j = (year + year / 4 + i) % 7;
                let e = if year <= 1600 {
                    10
                } else {
                    10 + year / 100 - 16 - (year / 100 - 16) / 4
                };
                (i - j + e) as u32
            },
            EasterType::Western => {
                let c = year / 100;
                let c_div_4 = c / 4;
                let h = (c - c_div_4 - (8 * c + 13) / 25 + 19 * g + 15) % 30;
                let h_div_28 = h / 28;
                let i = h - h_div_28 * (1 - h_div_28 * (29 / (h + 1)) * ((21 - g) / 11));
                let j = (year + year / 4 + i + 2 - c + c_div_4) % 7;
                (i - j) as u32
            }
        };
        
        let day = 1 + (p + 27 + (p + 6) / 40) % 31;
        let month = 3 + (p + 26) / 30;
        
        NaiveDate::from_ymd_opt(year, month, day)
    }
}

impl RecurringHoliday for EasterRelatedHoliday {
    fn get_holiday(&self, year: i32) -> HashSet<NaiveDate> {
        let mut holiday_set = HashSet::new();
        
        if let Some(easter_day) = self.get_easter_day(year) {
            holiday_set.insert(easter_day + self.shift_period);
        }
        
        holiday_set
    }
}
