use std::collections::HashSet;
use std::rc::Rc;

use chrono::{
    Datelike, 
    Days,
    NaiveDate, 
    Weekday
};

use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::recurringholiday::recurringholiday::RecurringHoliday;

/// Optimized weekend representation using bitmask
/// Each bit represents a day: Mon(0), Tue(1), ..., Sun(6)
#[derive(Clone, Copy)]
struct WeekendMask(u8);

impl WeekendMask {
    fn new(weekends: &HashSet<Weekday>) -> Self {
        let mut mask = 0u8;
        for &weekday in weekends {
            mask |= 1u8 << weekday.num_days_from_monday();
        }
        WeekendMask(mask)
    }

    #[inline]
    fn is_weekend(&self, weekday: Weekday) -> bool {
        let bit = 1u8 << weekday.num_days_from_monday();
        (self.0 & bit) != 0
    }

    /// Returns a list of weekend weekdays (for iteration optimization)
    fn weekend_list(&self) -> Vec<Weekday> {
        let mut weekdays = Vec::with_capacity(7);
        for day in 0..7 {
            if (self.0 & (1u8 << day)) != 0 {
                if let Ok(weekday) = Weekday::try_from(day as u8) {
                    weekdays.push(weekday);
                }
            }
        }
        weekdays
    }

    fn to_hashset(&self) -> HashSet<Weekday> {
        let mut set = HashSet::new();
        for day in 0..7 {
            if (self.0 & (1u8 << day)) != 0 {
                if let Ok(weekday) = Weekday::try_from(day as u8) {
                    set.insert(weekday);
                }
            }
        }
        set
    }
}

pub struct SimpleCalendar {
    weekends: WeekendMask,
    recurring_holidays: Vec<Rc<dyn RecurringHoliday>>,
    additional_holidays: HashSet<NaiveDate>,
    additional_business_days: HashSet<NaiveDate>
}

impl SimpleCalendar {
    /// Creates a new SimpleCalendar.
    /// 
    /// # Arguments
    /// * `weekends` - Set of weekdays that are considered weekends
    /// * `recurring_holidays` - List of recurring holiday rules
    /// * `additional_holidays` - One-time holidays (takes ownership)
    /// * `additional_business_days` - Special business days that override weekends/holidays
    pub fn new(
        weekends: HashSet<Weekday>,
        recurring_holidays: Vec<Rc<dyn RecurringHoliday>>,
        additional_holidays: Vec<NaiveDate>,
        additional_business_days: Vec<NaiveDate>
    ) -> SimpleCalendar {
        SimpleCalendar {
            weekends: WeekendMask::new(&weekends),
            recurring_holidays,
            additional_holidays: additional_holidays.into_iter().collect(),
            additional_business_days: additional_business_days.into_iter().collect()
        }
    }

    pub fn additional_business_days(&self) -> &HashSet<NaiveDate> {
        &self.additional_business_days
    }

    #[inline]
    pub fn is_weekend(&self, d: NaiveDate) -> bool {
        self.weekends.is_weekend(d.weekday())
    }

    pub fn is_recurring_holiday(&self, d: NaiveDate) -> bool {
        self.recurring_holidays.iter().any(|r| r.is_holiday(&d))
    }

    #[inline]
    pub fn is_additional_holiday(&self, d: NaiveDate) -> bool {
        self.additional_holidays.contains(&d)
    }

    #[inline]
    pub fn is_additional_business_day(&self, d: NaiveDate) -> bool {
        self.additional_business_days.contains(&d)
    }

    pub fn weekends(&self) -> HashSet<Weekday> {
        self.weekends.to_hashset()
    }
}

const SEVEN_DAYS: Days = Days::new(7);

impl HolidayCalendar for SimpleCalendar {
    fn is_holiday(&self, d: NaiveDate) -> bool {
        // Optimization: Check in order of likelihood
        // 1. Most common: weekends (~28% of days)
        if self.is_weekend(d) {
            // Weekend can be overridden by additional_business_day
            return !self.is_additional_business_day(d);
        }
        
        // 2. Early exit: if it's a special business day, it's not a holiday
        if self.is_additional_business_day(d) {
            return false;
        }
        
        // 3. Check additional holidays (small set, fast)
        if self.is_additional_holiday(d) {
            return true;
        }
        
        // 4. Last resort: check recurring holidays (requires computation)
        self.is_recurring_holiday(d)
    }

    /// Ultra-optimized: Returns all holidays in the given year, INCLUDING weekends.
    /// 
    /// Key optimization: Instead of iterating through all 365 days,
    /// we only iterate through actual weekend weekdays (e.g., Sat + Sun = ~104 iterations).
    /// 
    /// Algorithm:
    /// 1. For each weekend weekday (e.g., Saturday):
    ///    - Find the first occurrence in the year
    ///    - Add 7 days repeatedly until we exceed the year
    /// 2. Add recurring holidays
    /// 3. Add additional holidays
    /// 4. Remove business days
    fn get_holiday_set(&self, year: i32) -> HashSet<NaiveDate> {
        // Pre-allocate with estimated capacity
        // Typical year has ~104 weekends + recurring holidays
        let mut holiday_set = HashSet::with_capacity(120);
        
        // 1. Add all weekends using optimized iteration
        if self.weekends.0 != 0 {
            let weekend_days = self.weekends.weekend_list();
            let year_end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
            
            for target_weekday in weekend_days {
                // Find the first occurrence of this weekday in the year
                let mut current = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
                
                // Fast-forward to the first matching weekday
                while current.weekday() != target_weekday {
                    current = current.succ_opt().unwrap();
                }
                
                // Now iterate by adding 7 days until we exceed the year
                while current <= year_end {
                    holiday_set.insert(current);
                    current = current + SEVEN_DAYS;
                }
            }
        }

        // 2. Collect all recurring holidays using extend (no repeated allocations)
        for r in self.recurring_holidays.iter() {
            holiday_set.extend(r.get_holiday(year));
        }

        // 3. Add additional holidays for this year
        holiday_set.extend(
            self.additional_holidays
                .iter()
                .filter(|d| d.year() == year)
                .copied()
        );

        // 4. Remove special business days for this year (these override everything)
        for b_day in self.additional_business_days.iter().filter(|d| d.year() == year) {
            holiday_set.remove(b_day);
        }

        holiday_set
    }
}