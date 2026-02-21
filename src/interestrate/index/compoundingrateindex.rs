// ── compoundingrateindex.rs ──────────────────────────────────────────────────
//
// SOFR-style daily compounding index。
//
// # Observation conventions
//
// ## lookback_days（Lookback）
//   accrual day d → fixing_date = fixing_calendar.shift(d, -lookback_days)
//   每天各自往前移，fixing/accrual date 整體錯位。
//   SOFR lookback standard: lookback_days = 2。
//
// ## lockout_days（Lockout / Rate Lock）
//   期末最後 N 個 accrual days 全部鎖定，使用第 (n - lockout_days) 天的 fixing。
//   目的：讓 coupon 金額在 payment 前 N 天就可以確認，降低結算風險。
//   例：lockout_days = 2，period 共 5 天：
//     accrual: [d0, d1, d2, d3, d4]
//     fixing:  [f0, f1, f2, f2, f2]  ← d3, d4 鎖定用 f2
//
//   兩者獨立，可同時設定（SOFR ISDA 2020 standard = lookback 2 + no lockout）。
//
// # Arbitrage-Free vs Standard Forward
//
// 三個條件同時成立時，telescoping 成立，可用 D(start)/D(end) 替代逐日乘積：
//   - lookback_days == 0
//   - fixing_convention == Advance
//   - lockout_days == 0
//
// 切換：`index.set_use_arbitrage_free(bool)`。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use chrono::{Days, NaiveDate};

use super::compoundingconvention::{
    FixingConvention, MissingFixingFn, MissingFixingHandler,
    arbitrage_free_applicable, missing_fixing_fn_for,
};
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
    fixing_calendar: Arc<dyn HolidayCalendar>,
    day_counter: DayCounter,
    daily_past_fixings: HashMap<NaiveDate, f64>,
    result_compounding: Compounding,
    lookback_days: u32,
    lockout_days: u32,
    fixing_convention: FixingConvention,
    missing_fixing_handler: MissingFixingHandler,
    missing_fixing_fn: MissingFixingFn,
    arbitrage_free_applicable: bool,
    use_arbitrage_free: AtomicBool,
}

impl CompoundingRateIndex {
    /// 簡化建構式：無 shift、無 lockout、Advance、Null missing handler。
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
        Self::with_options(
            reference_curve_name, start_lag, adjuster, tenor,
            calendar.clone(), calendar,
            day_counter, daily_past_fixings, result_compounding,
            0, 0, FixingConvention::Advance, MissingFixingHandler::Null,
        )
    }

    pub fn with_options(
        reference_curve_name: String,
        start_lag: u32,
        adjuster: BusinessDayAdjuster,
        tenor: Period,
        calendar: Arc<dyn HolidayCalendar>,
        fixing_calendar: Arc<dyn HolidayCalendar>,
        day_counter: DayCounter,
        daily_past_fixings: HashMap<NaiveDate, f64>,
        result_compounding: Compounding,
        lookback_days: u32,
        lockout_days: u32,
        fixing_convention: FixingConvention,
        missing_fixing_handler: MissingFixingHandler,
    ) -> Self {
        let af_applicable = arbitrage_free_applicable(lookback_days, fixing_convention, lockout_days);
        Self {
            reference_curve_name,
            start_lag,
            adjuster,
            tenor,
            calendar,
            fixing_calendar,
            day_counter,
            daily_past_fixings,
            result_compounding,
            lookback_days,
            lockout_days,
            fixing_convention,
            missing_fixing_handler,
            missing_fixing_fn: missing_fixing_fn_for(missing_fixing_handler),
            arbitrage_free_applicable: af_applicable,
            use_arbitrage_free: AtomicBool::new(af_applicable),
        }
    }

    // ── 公開 accessors ────────────────────────────────────────────────────────

    pub fn arbitrage_free_applicable(&self) -> bool { self.arbitrage_free_applicable }
    pub fn lookback_days(&self) -> u32 { self.lookback_days }
    pub fn lockout_days(&self) -> u32 { self.lockout_days }
    pub fn fixing_convention(&self) -> FixingConvention { self.fixing_convention }
    pub fn missing_fixing_handler(&self) -> MissingFixingHandler { self.missing_fixing_handler }

    /// 切換 Arbitrage-Free 模式。
    /// 回傳實際生效的值（條件不滿足時強制為 false）。
    pub fn set_use_arbitrage_free(&self, enable: bool) -> bool {
        let effective = enable && self.arbitrage_free_applicable;
        self.use_arbitrage_free.store(effective, Ordering::Relaxed);
        effective
    }

    pub fn use_arbitrage_free(&self) -> bool {
        self.use_arbitrage_free.load(Ordering::Relaxed)
    }

    // ── 內部輔助 ─────────────────────────────────────────────────────────────

    /// 取得 [start, end) 之間所有業務日（accrual dates），按時間順序。
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

    /// 將 accrual day（index i，共 n 天）轉換為 fixing date。
    ///
    /// 優先順序：
    ///   1. Lockout：若 i >= n - lockout_days，取第 (n - lockout_days - 1) 天的 accrual date
    ///   2. FixingConvention：決定用 accrual start 還是 next accrual
    ///   3. Lookback：往前移 lookback_days 個業務日
    fn accrual_to_fixing(
        &self,
        business_days: &[NaiveDate],
        i: usize,
        end_date: NaiveDate,
    ) -> NaiveDate {
        let n = business_days.len();

        // Lockout：期末 lockout_days 天鎖定到第 (n - lockout_days - 1) 天
        let effective_i = if self.lockout_days > 0 && n > self.lockout_days as usize {
            i.min(n - self.lockout_days as usize - 1)
        } else {
            i
        };

        // FixingConvention：Advance 用 d_i，Arrear 用 d_{i+1}
        let base = match self.fixing_convention {
            FixingConvention::Advance => business_days[effective_i],
            FixingConvention::Arrear  => {
                business_days.get(effective_i + 1).copied().unwrap_or(end_date)
            }
        };

        // Lookback：往前移 lookback_days 個業務日
        if self.lookback_days == 0 {
            base
        } else {
            self.fixing_calendar.shift_n_business_day(base, -(self.lookback_days as i32))
        }
    }

    /// Standard Forward：∏(1 + r_i × δ_i)，逐日計算。
    fn standard_forward_factor(
        &self,
        business_days: &[NaiveDate],
        end_date: NaiveDate,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64 {
        business_days.iter().enumerate().fold(1.0, |acc, (i, &d)| {
            let next_d = business_days.get(i + 1).copied().unwrap_or(end_date);
            let tau    = self.day_counter.year_fraction(d, next_d);
            let fixing = self.accrual_to_fixing(business_days, i, end_date);
            // 用 fixing date 的 DF 比值推算 projected overnight rate
            let next_fixing = if i + 1 < business_days.len() {
                self.accrual_to_fixing(business_days, i + 1, end_date)
            } else {
                end_date
            };
            let rate = (forward_curve.discount(fixing) / forward_curve.discount(next_fixing) - 1.0) / tau;
            acc * (1.0 + rate * tau)
        })
    }

    /// Arbitrage-Free：D(start)/D(end)。
    /// 僅在 arbitrage_free_applicable 時呼叫（shift=0, Advance, lockout=0）。
    fn arbitrage_free_factor(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64 {
        forward_curve.discount(start) / forward_curve.discount(end)
    }

    /// 混合 past/future 的逐日計算（固定用 Standard Forward 計算 future 部分）。
    fn compute_compound_factor_mixed(
        &self,
        business_days: &[NaiveDate],
        end_date: NaiveDate,
        forward_curve: &Arc<dyn InterestRateCurve>,
        pricing_condition: &PricingCondition,
    ) -> f64 {
        business_days.iter().enumerate().fold(1.0, |acc, (i, &d)| {
            let next_d      = business_days.get(i + 1).copied().unwrap_or(end_date);
            let tau         = self.day_counter.year_fraction(d, next_d);
            let fixing_date = self.accrual_to_fixing(business_days, i, end_date);

            let is_past = fixing_date < *pricing_condition.horizon()
                || (fixing_date == *pricing_condition.horizon()
                    && !pricing_condition.estimate_horizon_index());

            let rate = if is_past {
                (self.missing_fixing_fn)(&self.daily_past_fixings, fixing_date)
            } else {
                // Projected：用 fixing date 的 DF 比值
                let next_fixing = if i + 1 < business_days.len() {
                    self.accrual_to_fixing(business_days, i + 1, end_date)
                } else {
                    end_date
                };
                (forward_curve.discount(fixing_date) / forward_curve.discount(next_fixing) - 1.0) / tau
            };

            acc * (1.0 + rate * tau)
        })
    }
}


impl InterestRateIndex for CompoundingRateIndex {

    fn start_lag(&self) -> u32 { self.start_lag }
    fn adjuster(&self) -> &BusinessDayAdjuster { &self.adjuster }
    fn tenor(&self) -> &Period { &self.tenor }
    fn calendar(&self) -> &Arc<dyn HolidayCalendar> { &self.calendar }
    fn day_counter(&self) -> &DayCounter { &self.day_counter }
    fn index_type(&self) -> InterestRateIndexType { InterestRateIndexType::CompoundingRate }
    fn reference_curve_name(&self) -> &String { &self.reference_curve_name }
    fn past_fixings(&self) -> &HashMap<NaiveDate, f64> { &self.daily_past_fixings }

    fn start_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        self.calendar.shift_n_business_day(fixing_date, -(self.start_lag as i32))
    }

    fn end_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        let start = self.start_date(fixing_date);
        self.adjuster.from_tenor_to_date(start, self.tenor, &self.calendar)
    }

    fn relative_dates_for_period(&self, period: &CalculationPeriod) -> HashSet<NaiveDate> {
        let mut dates: HashSet<NaiveDate> = self
            .business_days_in_period(period.start_date(), period.end_date())
            .into_iter()
            .collect();
        dates.insert(period.end_date());
        dates
    }

    fn projected_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64 {
        let tau = self.day_counter.year_fraction(period.start_date(), period.end_date());
        let compound_factor = if self.use_arbitrage_free() {
            // lockout_days == 0 已保證，telescoping 成立
            self.arbitrage_free_factor(period.start_date(), period.end_date(), forward_curve)
        } else {
            let bdays = self.business_days_in_period(period.start_date(), period.end_date());
            self.standard_forward_factor(&bdays, period.end_date(), forward_curve)
        };
        self.result_compounding.implied_rate(compound_factor, tau)
    }

    fn fixing_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let tau    = self.day_counter.year_fraction(period.start_date(), period.end_date());
        let bdays  = self.business_days_in_period(period.start_date(), period.end_date());
        let factor = self.compute_compound_factor_mixed(
            &bdays,
            period.end_date(),
            forward_curve_opt?,
            pricing_condition,
        );
        Some(self.result_compounding.implied_rate(factor, tau))
    }
}
