// ── compoundingrateindex.rs ──────────────────────────────────────────────────
//
// SOFR-style daily compounding index。
//
// # Stub 處理
//
// CompoundingRateIndex 天然支援任意長度的 accrual period（逐日計算），
// stub period 和正常 period 使用完全相同的計算路徑。
// `CalculationPeriod::is_stub()` 不需要在此判斷。
//
// # TermRateIndex vs CompoundingRateIndex（pure projection 下）
//
// 數學上 ∏ D(t_i)/D(t_{i+1}) = D(start)/D(end)，
// 兩者 projection 結果完全相同。
// 差異在 past fixing 的混合計算：CompoundingRateIndex 逐日判斷，
// 可正確處理橫跨 pricing_condition.horizon() 的期間。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{Days, NaiveDate};
use serde::Deserialize;

use super::super::compounding::Compounding;
use super::interestrateindex::{InterestRateIndex, InterestRateIndexType};
use super::super::super::model::interestrate::interestratecurve::InterestRateCurve;
use super::super::super::pricingcondition::PricingCondition;
use super::super::super::time::businessdayadjuster::BusinessDayAdjuster;
use super::super::super::time::calendar::holidaycalendar::HolidayCalendar;
use super::super::super::time::daycounter::daycounter::DayCounter;
use super::super::super::time::period::Period;
use super::super::super::time::schedule::scheduleperiod::CalculationPeriod;


pub struct CompoundingRateIndex {
    reference_curve_name: String,
    start_lag: u32,
    adjuster: BusinessDayAdjuster,
    tenor: Period,
    calendar: Arc<dyn HolidayCalendar>,
    day_counter: DayCounter,
    /// 每日 overnight rate past fixings。
    /// key = observation date（業務日），value = 當日實際 overnight rate
    daily_past_fixings: HashMap<NaiveDate, f64>,
    result_compounding: Compounding,
}

impl CompoundingRateIndex {
    pub fn new(
        reference_curve_name: String,
        start_lag: u32,
        adjuster: BusinessDayAdjuster,
        tenor: Period,
        calendar: Arc<dyn HolidayCalendar>,
        day_counter: DayCounter,
        daily_past_fixings: HashMap<NaiveDate, f64>,
        result_compounding: Compounding,
    ) -> Self {
        Self {
            reference_curve_name,
            start_lag,
            adjuster,
            tenor,
            calendar,
            day_counter,
            daily_past_fixings,
            result_compounding,
        }
    }

    /// 取得 [start, end) 之間所有業務日，按時間順序排列。
    fn business_days_in_period(&self, start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
        let mut dates = Vec::new();
        let mut d = start;
        while d < end {
            if self.calendar.is_business_day(d) {
                dates.push(d);
            }
            d += Days::new(1);
        }
        dates
    }

    /// 計算 compound factor = ∏(1 + r_i × δ_i)。
    ///
    /// 逐日判斷 past/future：
    ///   - past：使用 daily_past_fixings 的實際 overnight rate
    ///   - future：從 forward_curve 推算 D(t_i)/D(t_{next}) - 1) / δ_i
    fn compute_compound_factor(
        &self,
        business_days: &[NaiveDate],
        end_date: NaiveDate,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let mut factor = 1.0_f64;

        for (i, &t_i) in business_days.iter().enumerate() {
            let t_next = business_days.get(i + 1).copied().unwrap_or(end_date);
            let delta  = self.day_counter.year_fraction(t_i, t_next);

            let is_past = t_i < *pricing_condition.horizon()
                || (t_i == *pricing_condition.horizon()
                    && !pricing_condition.estimate_horizon_index());

            let overnight_rate = if is_past {
                *self.daily_past_fixings.get(&t_i)?
            } else {
                let curve = forward_curve_opt?;
                (curve.discount(t_i) / curve.discount(t_next) - 1.0) / delta
            };

            factor *= 1.0 + overnight_rate * delta;
        }

        Some(factor)
    }
}

impl InterestRateIndex for CompoundingRateIndex {

    fn start_lag(&self) -> u32 { self.start_lag }

    fn adjuster(&self) -> &BusinessDayAdjuster { &self.adjuster }

    fn tenor(&self) -> &Period { &self.tenor }

    fn calendar(&self) -> &Arc<dyn HolidayCalendar> { &self.calendar }

    fn day_counter(&self) -> &DayCounter { &self.day_counter }

    fn start_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        self.calendar.shift_n_business_day(fixing_date, -(self.start_lag as i32))
    }

    fn end_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        let start = self.start_date(fixing_date);
        self.adjuster.from_tenor_to_date(start, self.tenor, &self.calendar)
    }

    /// 回傳 period 內所有業務日加上 end_date。
    ///
    /// stub / non-stub 完全相同路徑：逐日計算天然支援任意長度。
    fn relative_dates_for_period(&self, period: &CalculationPeriod) -> HashSet<NaiveDate> {
        let mut dates: HashSet<NaiveDate> = self
            .business_days_in_period(period.start_date(), period.end_date())
            .into_iter()
            .collect();
        dates.insert(period.end_date());
        dates
    }

    /// Pure projection（完全使用 forward curve）。
    ///
    /// 數學上 ∏ D(t_i)/D(t_{i+1}) = D(start)/D(end)，
    /// 故直接用兩端的 DF 計算，效率與 TermRateIndex 相同。
    fn projected_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64 {
        let tau = self.day_counter.year_fraction(period.start_date(), period.end_date());
        let fv  = forward_curve.discount(period.start_date())
                / forward_curve.discount(period.end_date());
        self.result_compounding.implied_rate(fv, tau)
    }

    /// 混合計算：逐日判斷 past/future。
    ///
    /// stub period 和正常 period 完全相同路徑，不需要 is_stub() 判斷。
    fn fixing_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let tau           = self.day_counter.year_fraction(period.start_date(), period.end_date());
        let business_days = self.business_days_in_period(period.start_date(), period.end_date());
        let compound_factor = self.compute_compound_factor(
            &business_days,
            period.end_date(),
            forward_curve_opt,
            pricing_condition,
        )?;
        Some(self.result_compounding.implied_rate(compound_factor, tau))
    }

    fn index_type(&self) -> InterestRateIndexType { InterestRateIndexType::CompoundingRate }

    fn reference_curve_name(&self) -> &String { &self.reference_curve_name }

    fn past_fixings(&self) -> &HashMap<NaiveDate, f64> { &self.daily_past_fixings }
}
