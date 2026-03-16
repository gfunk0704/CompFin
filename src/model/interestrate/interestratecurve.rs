use std::sync::Arc;

use chrono::NaiveDate;

use crate::time::daycounter::daycounter::DayCounter;


pub trait InterestRateCurve: Send + Sync {
    fn day_counter(&self) -> Arc<DayCounter>;

    fn reference_date(&self) -> NaiveDate;

    fn discount(&self, d: NaiveDate) -> f64;

    #[inline]
    fn zero_rate(&self, d: NaiveDate) -> f64 {
        let t = self.year_fraction(d);
        -self.discount(d).ln() / t
    }

    #[inline]
    fn year_fraction(&self, d: NaiveDate) -> f64 {
        self.day_counter().year_fraction(self.reference_date(), d)
    }
}
