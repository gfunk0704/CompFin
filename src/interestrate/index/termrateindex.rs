use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::NaiveDate;

use super::super::compounding::Compounding;
use super::interestrateindex::{InterestRateIndex, InterestRateIndexType};
use super::super::super::model::interestrate::interestratecurve::InterestRateCurve;
use super::super::super::pricingcondition::PricingCondition;
use super::super::super::time::businessdayadjuster::BusinessDayAdjuster;
use super::super::super::time::calendar::holidaycalendar::HolidayCalendar;
use super::super::super::time::daycounter::daycounter::DayCounter;
use super::super::super::time::period::Period;
use super::super::super::time::schedule::scheduleperiod::CalculationPeriod;


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
    ) -> Self {
        Self {
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

    /// 從 start_date 反推 fixing_date。
    pub fn fixing_date_from_start(&self, start_date: NaiveDate) -> NaiveDate {
        self.calendar.shift_n_business_day(start_date, self.start_lag as i32)
    }
}

impl InterestRateIndex for TermRateIndex {

    fn start_lag(&self) -> u32 { self.start_lag }
    fn adjuster(&self) -> &BusinessDayAdjuster { &self.adjuster }
    fn tenor(&self) -> &Period { &self.tenor }
    fn calendar(&self) -> &Arc<dyn HolidayCalendar> { &self.calendar }
    fn day_counter(&self) -> &DayCounter { &self.day_counter }
    fn index_type(&self) -> InterestRateIndexType { InterestRateIndexType::TermRate }
    fn reference_curve_name(&self) -> &String { &self.reference_curve_name }
    fn past_fixings(&self) -> &HashMap<NaiveDate, f64> { &self.past_fixings }

    fn start_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        self.calendar.shift_n_business_day(fixing_date, -(self.start_lag as i32))
    }

    fn end_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        let start = self.start_date(fixing_date);
        self.adjuster.from_tenor_to_date(start, self.tenor, &self.calendar)
    }

    /// Projection：curve 連續，任意 period 都用相同公式。
    /// stub/non-stub 完全相同路徑，不需要 is_stub() 判斷。
    fn projected_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64 {
        let tau = self.day_counter.year_fraction(period.start_date(), period.end_date());
        let fv  = forward_curve.discount(period.start_date())
                / forward_curve.discount(period.end_date());
        self.compounding.implied_rate(fv, tau)
    }

    fn relative_dates_for_period(&self, period: &CalculationPeriod) -> HashSet<NaiveDate> {
        HashSet::from_iter([period.start_date(), period.end_date()])
    }

    /// Past fixing 查詢：直接以 start_date 反推 fixing_date 查 past_fixings。
    ///
    /// Stub convention（Straight / Interpolation / Proportional）的邏輯
    /// 不在此處，由上層的 TermRateCalculator 決定傳入哪個 period。
    /// 例如 Proportional 時，TermRateCalculator 會把 regular period 傳進來，
    /// 再把查到的 rate 乘以比例後回傳給 leg。
    fn fixing_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let is_past = period.start_date() < *pricing_condition.horizon()
            || (period.start_date() == *pricing_condition.horizon()
                && !pricing_condition.estimate_horizon_index());

        if is_past {
            let fixing_date = self.fixing_date_from_start(period.start_date());
            self.past_fixings.get(&fixing_date).copied()
        } else {
            Some(self.projected_rate_for_period(period, forward_curve_opt.unwrap()))
        }
    }
}
