use std::collections::HashMap;

use chrono::{
    Datelike, 
    NaiveDate, 
    Weekday
};
use serde::{
    Serialize,
    Deserialize
};

use super::super::period::Period;


#[derive(PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum WeekendAdjustment {
    Unadjusted = 0,
    NextWeekday = 1,
    PreviousWeekday = -1
}

/// Array-based weekend adjustment rule for O(1) lookup performance.
/// Uses a fixed-size array indexed by weekday (0=Monday, 6=Sunday).
#[derive(Clone)]
pub struct WeekendAdjustmentRule {
    // Array indexed by Weekday::num_days_from_monday() (0-6)
    // None means no adjustment needed for that weekday
    rule: [Option<Period>; 7]
}

impl WeekendAdjustmentRule {
    /// Creates a new WeekendAdjustmentRule from a HashMap.
    /// 
    /// This processes the adjustment map to calculate the actual shift needed
    /// when multiple consecutive days are marked as weekends.
    pub fn new(adjustment_map: &HashMap<Weekday, WeekendAdjustment>) -> WeekendAdjustmentRule {
        let mut rule: [Option<Period>; 7] = [None; 7];
        
        for (&weekday, &adj) in adjustment_map {
            if adj == WeekendAdjustment::Unadjusted {
                continue;
            }

            let mut to_weekday = weekday;
            let next_weekday = if adj == WeekendAdjustment::NextWeekday {
                Weekday::succ
            } else {
                Weekday::pred
            };
            
            let mut shift_days = 0;
            let one_day = adj as i32;
            
            // Find the next non-weekend day by following the adjustment direction
            while adjustment_map.contains_key(&to_weekday) {
                to_weekday = next_weekday(&to_weekday);
                shift_days += one_day;
            }
            
            let idx = weekday.num_days_from_monday() as usize;
            rule[idx] = Some(Period::days(shift_days));
        }
        
        WeekendAdjustmentRule { rule }
    }

    /// Reconstructs the adjustment map from the internal rule array.
    /// Useful for serialization or debugging.
    pub fn adjustment_map(&self) -> HashMap<Weekday, WeekendAdjustment> {
        let mut result: HashMap<Weekday, WeekendAdjustment> = HashMap::new();
        
        // Iterate through all weekdays
        for day_offset in 0..7 {
            if let Some(period) = self.rule[day_offset] {
                let weekday = Weekday::try_from(day_offset as u8).unwrap();
                
                let adjustment = match period.number() {
                    n if n > 0 => WeekendAdjustment::NextWeekday,
                    n if n < 0 => WeekendAdjustment::PreviousWeekday,
                    _ => WeekendAdjustment::Unadjusted,
                };
                
                result.insert(weekday, adjustment);
            }
        }
        
        result
    }

    /// Adjusts a date according to the weekend adjustment rules.
    /// 
    /// This is an O(1) operation using array indexing.
    #[inline]
    pub fn adjust(&self, d: NaiveDate) -> NaiveDate {
        let idx = d.weekday().num_days_from_monday() as usize;
        
        match self.rule[idx] {
            Some(period) => d + period,
            None => d,
        }
    }
}
