use std::sync::Arc;

use crate::market::market::Market;
use crate::market::currency::Currency;
use crate::time::calendar::holidaycalendar::HolidayCalendar;

pub struct SingleCurrcneyMarket {
    discount_curve_name: String,
    settlement_calendar: Arc<dyn HolidayCalendar>,
    settlement_currency: Currency,
    settlement_days: u32
}

impl SingleCurrcneyMarket {
    pub fn new(discount_curve_name: String,
               settlement_calendar: Arc<dyn HolidayCalendar>,
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

    fn settlement_calendar(&self) -> Arc<(dyn HolidayCalendar + 'static)> {
        Arc::clone(&self.settlement_calendar)
    }

    fn settlement_currency(&self) -> &Currency {
        &self.settlement_currency
    }

    fn settlement_days(&self) -> u32 {
        self.settlement_days
    }
}