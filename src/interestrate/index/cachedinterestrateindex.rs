// ── cached_interest_rate_index.rs ───────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::NaiveDate;

use super::cachebackend::{CacheBackend, RefCellBackend, RwLockBackend};
use super::interestrateindex::{InterestRateIndex, InterestRateIndexType};
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounter;
use crate::time::period::Period;
use crate::time::schedule::scheduleperiod::CalculationPeriod;

/// 核心 struct：C 決定執行緒安全性（RefCellBackend 或 RwLockBackend）。
pub struct CachedInterestRateIndex<C: CacheBackend> {
    index: Arc<dyn InterestRateIndex + Send + Sync>,
    backend: C,
}

impl<C: CacheBackend> CachedInterestRateIndex<C> {
    fn new_with_backend(
        index: Arc<dyn InterestRateIndex + Send + Sync>,
        backend: C,
    ) -> Self {
        Self { index, backend }
    }
}

impl CachedInterestRateIndex<RefCellBackend> {
    /// 單執行緒版（RefCell，無鎖）。
    pub fn new(index: Arc<dyn InterestRateIndex + Send + Sync>) -> Self {
        Self::new_with_backend(index, RefCellBackend::new())
    }
}

impl CachedInterestRateIndex<RwLockBackend> {
    /// 多執行緒版（RwLock）。
    pub fn new_threadsafe(index: Arc<dyn InterestRateIndex + Send + Sync>) -> Self {
        Self::new_with_backend(index, RwLockBackend::new())
    }
}

impl<C: CacheBackend> InterestRateIndex for CachedInterestRateIndex<C> {

    // ── 靜態屬性 delegate ────────────────────────────────────────────────────

    fn adjuster(&self) -> &BusinessDayAdjuster        { self.index.adjuster() }
    fn calendar(&self) -> &Arc<dyn HolidayCalendar>   { self.index.calendar() }
    fn start_lag(&self) -> u32                         { self.index.start_lag() }
    fn tenor(&self) -> &Period                         { self.index.tenor() }
    fn start_date(&self, d: NaiveDate) -> NaiveDate    { self.index.start_date(d) }
    fn end_date(&self, d: NaiveDate) -> NaiveDate      { self.index.end_date(d) }
    fn day_counter(&self) -> &DayCounter               { self.index.day_counter() }
    fn index_type(&self) -> InterestRateIndexType      { self.index.index_type() }
    fn reference_curve_name(&self) -> &String          { self.index.reference_curve_name() }
    fn past_fixings(&self) -> &HashMap<NaiveDate, f64> { self.index.past_fixings() }

    // ── projected_rate_for_period：唯一快取的計算路徑 ────────────────────────
    //
    // Cache key = (curve_ptr, period.start_date())
    //
    // start_date 作為 key 足夠，因為：
    //   - TermRateIndex：同一 index 下，start 由 tenor 唯一決定 end
    //   - CompoundingRateIndex：同一 accrual period 的 start 唯一決定 end
    //
    // Arc<dyn Trait> 是 fat pointer，需先轉 *const () 取資料指標再轉 usize。
    // 同一個 Arc（或其 clone）資料指標相同 → cache 命中；新 curve 物件 → cache 清除。
    //
    // fixing_rate_for_period 不做快取：
    //   mixed past/future 的結果隨 pricing_condition.horizon() 改變，
    //   不適合用 (curve_ptr, start_date) 作為 key。

    fn projected_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64 {
        let curve_ptr = Arc::as_ptr(forward_curve) as *const () as usize;
        self.backend.get_or_compute(curve_ptr, period.start_date(), || {
            self.index.projected_rate_for_period(period, forward_curve)
        })
    }

    fn relative_dates_for_period(&self, period: &CalculationPeriod) -> HashSet<NaiveDate> {
        self.index.relative_dates_for_period(period)
    }

    fn fixing_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        self.index.fixing_rate_for_period(period, forward_curve_opt, pricing_condition)
    }
}

// ── Type alias ───────────────────────────────────────────────────────────────

pub type SingleThreadedCachedIndex = CachedInterestRateIndex<RefCellBackend>;
pub type MultiThreadedCachedIndex  = CachedInterestRateIndex<RwLockBackend>;
