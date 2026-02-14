use std::rc::Rc;

use chrono::NaiveDate;

use crate::objectwithuuid::ObjectWithUUID;
use crate::time::daycounter::daycounter::DayCounter;


pub trait InterestRateCurve: ObjectWithUUID {
    fn day_counter(&self) -> Rc<DayCounter>;

    fn reference_date(&self) -> NaiveDate;

    fn discount(&self, d: NaiveDate) -> f64;

    fn zero_rate(&self, d: NaiveDate) -> f64 {
        let t = self.day_counter().year_fraction(self.reference_date(), d);
        -self.discount(d).ln() / t
    }
}
