use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::NaiveDate;
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


// ─────────────────────────────────────────────────────────────────────────────
// StubRateConvention
// ─────────────────────────────────────────────────────────────────────────────

/// Stub period 的 past fixing 計算慣例（僅對 TermRateIndex 有意義）。
///
/// Projection 下不需要特殊處理：forward curve 是連續的，
/// `D(stub_start)/D(stub_end)` 對任意區間都正確。
///
/// 慣例差異只出現在 **past fixing** 查詢：
/// 當 stub period 已成為過去，無法直接查到 stub 長度的歷史 fixing 時
/// （例如查不到 40-day rate），需要從已知 tenor 的 fixings 推算。
///
/// 參考：ISDA 2006, CS Lucas user guide
#[derive(Clone, Copy, Deserialize)]
pub enum StubRateConvention {
    /// 直接使用與 stub 最接近的 index tenor 的 fixing。
    /// 例如 short 6M stub 在 1Y index 下，直接使用 1Y fixing。
    /// 操作最簡單，但有時間錯配。
    Straight,

    /// 在兩個鄰近 tenor 之間線性插值。
    /// 例如 40-day stub 在 1M/3M 之間插值。
    /// 需要額外配置 `short_tenor` / `long_tenor` 供插值使用。
    Interpolation {
        short_tenor: Period,
        long_tenor: Period,
    },

    /// 按天數比例縮放：`rate × (stub_days / regular_days)`。
    /// 例如 40-day stub 對應 3M (91-day) tenor：`rate × (40/91)`。
    /// 適用於 short stub 且差距不大的場景。
    Proportional,
}

impl Default for StubRateConvention {
    fn default() -> Self {
        StubRateConvention::Straight
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// TermRateIndex
// ─────────────────────────────────────────────────────────────────────────────

pub struct TermRateIndex {
    reference_curve_name: String,
    start_lag: u32,
    adjuster: BusinessDayAdjuster,
    tenor: Period,
    calendar: Arc<dyn HolidayCalendar>,
    day_counter: DayCounter,
    compounding: Compounding,
    past_fixings: HashMap<NaiveDate, f64>,
    stub_rate_convention: StubRateConvention,
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
        Self::with_stub_convention(
            reference_curve_name,
            start_lag,
            adjuster,
            tenor,
            calendar,
            day_counter,
            compounding,
            past_fixings,
            StubRateConvention::default(),
        )
    }

    pub fn with_stub_convention(
        reference_curve_name: String,
        start_lag: u32,
        adjuster: BusinessDayAdjuster,
        tenor: Period,
        calendar: Arc<dyn HolidayCalendar>,
        day_counter: DayCounter,
        compounding: Compounding,
        past_fixings: HashMap<NaiveDate, f64>,
        stub_rate_convention: StubRateConvention,
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
            stub_rate_convention,
        }
    }

    // ── stub past fixing 計算 ─────────────────────────────────────────────

    /// 從 start_date 反推 fixing_date（加回 start_lag 個業務日）。
    fn fixing_date_from_start(&self, start_date: NaiveDate) -> NaiveDate {
        self.calendar.shift_n_business_day(start_date, self.start_lag as i32)
    }

    /// Straight：直接查 self.tenor 的 fixing（以 regular_start 對應的 fixing_date 查詢）。
    fn stub_rate_straight(&self, period: &CalculationPeriod) -> Option<f64> {
        let fixing_date = self.fixing_date_from_start(period.regular_start_date());
        self.past_fixings.get(&fixing_date).copied()
    }

    /// Interpolation：在 short_tenor / long_tenor 兩個 fixings 之間線性插值。
    ///
    /// 插值公式（天數線性）：
    ///   rate = short_rate + (long_rate - short_rate)
    ///          × (stub_days - short_days) / (long_days - short_days)
    fn stub_rate_interpolated(
        &self,
        period: &CalculationPeriod,
        short_tenor: Period,
        long_tenor: Period,
    ) -> Option<f64> {
        let fixing_date = self.fixing_date_from_start(period.start_date());

        // 從 fixing_date 推算 short/long tenor 的 end date
        let short_start = period.start_date();
        let long_start  = period.start_date();
        let short_end   = self.adjuster.from_tenor_to_date(short_start, short_tenor, &self.calendar);
        let long_end    = self.adjuster.from_tenor_to_date(long_start,  long_tenor,  &self.calendar);

        // 對應的 fixing dates（short / long tenor 各自的 fixing_date）
        let short_fixing = self.fixing_date_from_start(short_start);
        let long_fixing  = self.fixing_date_from_start(long_start);

        let short_rate = *self.past_fixings.get(&short_fixing)?;
        let long_rate  = *self.past_fixings.get(&long_fixing)?;

        // 以 year_fraction 換算天數比（保持 day count convention 一致）
        let stub_tau  = self.day_counter.year_fraction(period.start_date(), period.end_date());
        let short_tau = self.day_counter.year_fraction(short_start, short_end);
        let long_tau  = self.day_counter.year_fraction(long_start, long_end);

        if (long_tau - short_tau).abs() < 1e-10 {
            return Some(short_rate);
        }

        let weight = (stub_tau - short_tau) / (long_tau - short_tau);
        Some(short_rate + weight * (long_rate - short_rate))
    }

    /// Proportional：`regular_rate × (stub_tau / regular_tau)`。
    fn stub_rate_proportional(&self, period: &CalculationPeriod) -> Option<f64> {
        let fixing_date   = self.fixing_date_from_start(period.regular_start_date());
        let regular_rate  = *self.past_fixings.get(&fixing_date)?;

        let stub_tau    = self.day_counter.year_fraction(period.start_date(),         period.end_date());
        let regular_tau = self.day_counter.year_fraction(period.regular_start_date(), period.regular_end_date());

        if regular_tau.abs() < 1e-10 {
            return Some(regular_rate);
        }

        Some(regular_rate * (stub_tau / regular_tau))
    }

    /// 根據 `stub_rate_convention` 計算 stub 的 past fixing rate。
    fn stub_past_rate(&self, period: &CalculationPeriod) -> Option<f64> {
        match self.stub_rate_convention {
            StubRateConvention::Straight => self.stub_rate_straight(period),
            StubRateConvention::Interpolation { short_tenor, long_tenor } => {
                self.stub_rate_interpolated(period, short_tenor, long_tenor)
            }
            StubRateConvention::Proportional => self.stub_rate_proportional(period),
        }
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateIndex impl
// ─────────────────────────────────────────────────────────────────────────────

impl InterestRateIndex for TermRateIndex {

    fn start_lag(&self) -> u32 { self.start_lag }

    fn adjuster(&self) -> &BusinessDayAdjuster { &self.adjuster }

    fn tenor(&self) -> &Period { &self.tenor }

    fn start_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        self.calendar.shift_n_business_day(fixing_date, -(self.start_lag as i32))
    }

    fn end_date(&self, fixing_date: NaiveDate) -> NaiveDate {
        let start = self.start_date(fixing_date);
        self.adjuster.from_tenor_to_date(start, self.tenor, &self.calendar)
    }

    fn calendar(&self) -> &Arc<dyn HolidayCalendar> { &self.calendar }

    fn day_counter(&self) -> &DayCounter { &self.day_counter }

    fn projected_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64 {
        // Projection 下 curve 是連續的，stub/non-stub 完全相同的計算路徑。
        // D(start)/D(end) 對任意區間都正確，不需要 is_stub() 判斷。
        let tau = self.day_counter.year_fraction(period.start_date(), period.end_date());
        let fv  = forward_curve.discount(period.start_date())
                / forward_curve.discount(period.end_date());
        self.compounding.implied_rate(fv, tau)
    }

    fn relative_dates_for_period(&self, period: &CalculationPeriod) -> HashSet<NaiveDate> {
        HashSet::from_iter([period.start_date(), period.end_date()])
    }

    fn fixing_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        // past/future 判斷以 start_date 為代表（fixing date 在 start 前幾天）
        let is_past = period.start_date() < *pricing_condition.horizon()
            || (period.start_date() == *pricing_condition.horizon()
                && !pricing_condition.estimate_horizon_index());

        if is_past {
            if period.is_stub() {
                // stub：依 convention 計算 past rate
                self.stub_past_rate(period)
            } else {
                // 正常 period：直接查 fixing_date 的 past fixing
                let fixing_date = self.fixing_date_from_start(period.start_date());
                self.past_fixings.get(&fixing_date).copied()
            }
        } else {
            Some(self.projected_rate_for_period(period, forward_curve_opt.unwrap()))
        }
    }

    fn index_type(&self) -> InterestRateIndexType { InterestRateIndexType::TermRate }

    fn reference_curve_name(&self) -> &String { &self.reference_curve_name }

    fn past_fixings(&self) -> &HashMap<NaiveDate, f64> { &self.past_fixings }
}
