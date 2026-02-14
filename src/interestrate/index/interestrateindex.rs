use std::collections::{HashMap, HashSet};
use std::rc::Arc;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounter;
use crate::time::period::Period;


#[derive(PartialEq, Eq, Deserialize)]
pub enum InterestRateIndexType {
    TermRate,
}

pub trait InterestRateIndex {
    fn adjuster(&self) -> &BusinessDayAdjuster;

    fn calendar(&self) -> &Arc<dyn HolidayCalendar>;

    fn start_lag(&self) -> u32;

    fn tenor(&self) -> &Period;

    fn start_date(&self, fixing_date: NaiveDate) -> NaiveDate;

    fn end_date(&self, fixing_date: NaiveDate) -> NaiveDate;

    fn day_counter(&self) -> &DayCounter;

    fn relative_dates(&self, fixing_date: NaiveDate) -> HashSet<NaiveDate>;

    fn projected_rate(
        &self,
        fixing_date: NaiveDate,
        forward_curve: &Rc<dyn InterestRateCurve>,
    ) -> f64;

    fn index_type(&self) -> InterestRateIndexType;

    fn reference_curve_name(&self) -> &String;

    fn past_fixings(&self) -> &HashMap<NaiveDate, f64>;

    // ── Default 實作 ──────────────────────────────────────────────────────

    fn fixing_rate(
        &self,
        fixing_date: NaiveDate,
        forward_curve_opt: Option<&Rc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let is_past = fixing_date < *pricing_condition.horizon()
            || (fixing_date == *pricing_condition.horizon()
                && !pricing_condition.estimate_horizon_index());

        if is_past {
            self.past_fixings().get(&fixing_date).copied()
        } else {
            Some(self.projected_rate(fixing_date, forward_curve_opt.unwrap()))
        }
    }
}