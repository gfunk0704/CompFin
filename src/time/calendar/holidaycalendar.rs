use std::collections::HashSet;

use chrono::{Days, NaiveDate};

use crate::time::utility::days_of_month;

const ONE_DAY: Days = Days::new(1);
pub trait HolidayCalendar {
    fn is_holiday(&self, d: NaiveDate) -> bool;

    fn get_holiday_set(&self, year: i32) -> HashSet<NaiveDate>;

    fn is_business_day(&self, d: NaiveDate) -> bool {
        !self.is_holiday(d)
    }

    fn shift_n_business_day(&self, horizon: NaiveDate, n: i32) -> NaiveDate {
        const ONE_DAY: Days = Days::new(1);
        let shif_one_day = if n >= 0  {
            |d: NaiveDate| d + ONE_DAY
        } else {
             |d: NaiveDate| d - ONE_DAY
        };

        let mut m = n.abs() as u32;
        let mut d = horizon;
        while m > 0 {
            d = shif_one_day(d);
            m -= self.is_business_day(d) as u32;
        }   
        d
    }

    fn next_business_day(&self, d: NaiveDate) -> NaiveDate {
        self.shift_n_business_day(d, 1)
    }

    fn previous_business_day(&self, d: NaiveDate) -> NaiveDate {
        self.shift_n_business_day(d, -1)
    }

    fn last_business_day_of_month(&self, year: i32, month: u32) -> NaiveDate {
        let mut eom = NaiveDate::from_ymd_opt(year, month, days_of_month(year, month)).unwrap();
        while self.is_holiday(eom) {
            eom = eom - ONE_DAY;
        }
        eom
    }

    fn first_business_day_of_month(&self, year: i32, month: u32) -> NaiveDate {
        let mut fom = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        while self.is_holiday(fom) {
            fom = fom + ONE_DAY;
        }
        fom
    }
}