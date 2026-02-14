use std::collections::{HashMap, HashSet};
use std::rc::Arc;

use chrono::NaiveDate;

use super::super::compounding::Compounding;
use super::interestrateindex::{InterestRateIndex, InterestRateIndexType};
use super::super::super::model::interestrate::interestratecurve::InterestRateCurve;
use super::super::super::time::businessdayadjuster::BusinessDayAdjuster;
use super::super::super::time::calendar::holidaycalendar::HolidayCalendar;
use super::super::super::time::daycounter::daycounter::DayCounter;
use super::super::super::time::period::Period;


pub struct TermRateIndex {
    reference_curve_name: String,
    start_lag: u32,
    adjuster: BusinessDayAdjuster,
    tenor: Period,
    calendar: Arc<dyn HolidayCalendar>,
    day_counter: DayCounter,
    compounding: Compounding,
    past_fixings: HashMap<NaiveDate, f64>,
}

impl TermRateIndex {
    pub fn new(
        reference_curve_name: String,
        start_lag: u32,
        adjuster: BusinessDayAdjuster,
        tenor: Period,
        calendar: Arc<dyn HolidayCalendar>,
        day_counter: DayCounter,
        compounding: Compounding,
        past_fixings: HashMap<NaiveDate, f64>,
    ) -> TermRateIndex {
        TermRateIndex {
            reference_curve_name,
            start_lag,
            adjuster,
            tenor,
            calendar,
            day_counter,
            compounding,
            past_fixings,
        }
    }
}

impl InterestRateIndex for TermRateIndex {
    fn start_lag(&self) -> u32 {
        self.start_lag
    }

    fn adjuster(&self) -> &BusinessDayAdjuster {
        &self.adjuster
    }

    fn tenor(&self) -> &Period {
        &self.tenor
    }

    fn start_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        self.calendar.shift_n_business_day(fixing_date, -(self.start_lag as i32))
    }

    fn end_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        let start_date = self.start_date(fixing_date);
        self.adjuster.from_tenor_to_date(start_date, self.tenor, &self.calendar)
    }

    fn relative_dates(&self, fixing_date: NaiveDate) -> HashSet<NaiveDate> {
        HashSet::from_iter([
            self.start_date(fixing_date),
            self.end_date(fixing_date),
        ])
    }

    fn calendar(&self) -> &Rc<dyn HolidayCalendar> {
        &self.calendar
    }

    fn day_counter(&self) -> &DayCounter {
        &self.day_counter
    }

    fn projected_rate(
        &self,
        fixing_date: NaiveDate,
        forward_curve: &Rc<dyn InterestRateCurve>,
    ) -> f64 {
        let start_date   = self.start_date(fixing_date);
        let end_date     = self.end_date(fixing_date);
        let tau          = self.day_counter.year_fraction(start_date, end_date);
        let future_value = forward_curve.discount(start_date) / forward_curve.discount(end_date);
        self.compounding.implied_rate(future_value, tau)
    }

    fn index_type(&self) -> InterestRateIndexType {
        InterestRateIndexType::TermRate
    }

    fn reference_curve_name(&self) -> &String {
        &self.reference_curve_name
    }

    fn past_fixings(&self) -> &HashMap<NaiveDate, f64> {
        &self.past_fixings
    }
}