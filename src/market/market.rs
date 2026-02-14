use std::rc::Rc;

use chrono::NaiveDate;

use super::currency::Currency;
use super::super::time::calendar::holidaycalendar::HolidayCalendar;

pub trait Market {
    fn discount_curve_name(&self) -> &String;

    fn settlement_calendar(&self) -> &Rc<dyn HolidayCalendar>;

    fn settlement_currency(&self) -> &Currency;

    fn settlement_days(&self) -> u32;

    fn settlement_date(&self, horizon: NaiveDate) -> NaiveDate {
        self.settlement_calendar().
            shift_n_business_day(horizon, self.settlement_days() as i32)
    }
}


pub struct SingleCurrcneyMarket {
    discount_curve_name: String,
    settlement_calendar: Rc<dyn HolidayCalendar>,
    settlement_currency: Currency,
    settlement_days: u32
}

impl SingleCurrcneyMarket {
    pub fn new(discount_curve_name: String,
               settlement_calendar: Rc<dyn HolidayCalendar>,
               settlement_currency: Currency,
               settlement_days: u32) -> SingleCurrcneyMarket {
        SingleCurrcneyMarket {
            discount_curve_name: discount_curve_name,
            settlement_calendar: settlement_calendar,
            settlement_currency: settlement_currency,
            settlement_days: settlement_days
        }
    }
}

impl Market for SingleCurrcneyMarket {
    fn discount_curve_name(&self) -> &String {
        &self.discount_curve_name
    }

    fn settlement_calendar(&self) -> &Rc<dyn HolidayCalendar> {
        &self.settlement_calendar
    }

    fn settlement_currency(&self) -> &Currency {
        &self.settlement_currency
    }

    fn settlement_days(&self) -> u32 {
        self.settlement_days
    }
}