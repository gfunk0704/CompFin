use std::sync::Arc;

use chrono::NaiveDate;

use super::currency::Currency;
use super::super::time::calendar::holidaycalendar::HolidayCalendar;

pub trait Market : Send + Sync{
    fn discount_curve_name(&self) -> &String;

    fn settlement_calendar(&self) -> Arc<dyn HolidayCalendar>;

    fn expiry_calendar(&self) -> Arc<dyn HolidayCalendar> {
        self.settlement_calendar()
    }

    fn settlement_currency(&self) -> &Currency;

    fn settlement_days(&self) -> u32;

    fn settlement_date(&self, horizon: NaiveDate) -> NaiveDate {
        self.settlement_calendar().
            shift_n_business_day(horizon, self.settlement_days() as i32)
    }
}

