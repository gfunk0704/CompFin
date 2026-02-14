// ── cached_interest_rate_index.rs ───────────────────────────────────────────

use std::collections::{
    HashMap, 
    HashSet
};
use std::sync::Arc;

use chrono::NaiveDate;

use super::cachebackend::{
    CacheBackend, 
    RefCellBackend, 
    RwLockBackend
};
use super::interestrateindex::{InterestRateIndex, InterestRateIndexType};
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounter;
use crate::time::period::Period;

/// 核心 struct 只寫一次，C 決定執行緒安全性
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

/// 兩個 convenience constructor，分別對應兩種用途
impl CachedInterestRateIndex<RefCellBackend> {
    pub fn new(index: Arc<dyn InterestRateIndex + Send + Sync>) -> Self {
        Self::new_with_backend(index, RefCellBackend::new())
    }
}

impl CachedInterestRateIndex<RwLockBackend> {
    pub fn new_threadsafe(index: Arc<dyn InterestRateIndex + Send + Sync>) -> Self {
        Self::new_with_backend(index, RwLockBackend::new())
    }
}

/// InterestRateIndex 只需要實作一次
impl<C: CacheBackend> InterestRateIndex for CachedInterestRateIndex<C> {

    // ── delegate 方法 ────────────────────────────────────────────────────────

    fn adjuster(&self) -> &BusinessDayAdjuster       { self.index.adjuster() }
    fn calendar(&self) -> &Arc<dyn HolidayCalendar>  { self.index.calendar() }
    fn start_lag(&self) -> u32                        { self.index.start_lag() }
    fn tenor(&self) -> &Period                        { self.index.tenor() }
    fn start_date(&self, d: NaiveDate) -> NaiveDate   { self.index.start_date(d) }
    fn end_date(&self, d: NaiveDate) -> NaiveDate     { self.index.end_date(d) }
    fn day_counter(&self) -> &DayCounter              { self.index.day_counter() }
    fn index_type(&self) -> InterestRateIndexType     { self.index.index_type() }
    fn reference_curve_name(&self) -> &String         { self.index.reference_curve_name() }
    fn past_fixings(&self) -> &HashMap<NaiveDate, f64>{ self.index.past_fixings() }

    fn relative_dates(&self, d: NaiveDate) -> HashSet<NaiveDate> {
        self.index.relative_dates(d)
    }

    // ── 快取邏輯：完全委託給 backend ─────────────────────────────────────────

    fn projected_rate(
        &self,
        fixing_date: NaiveDate,
        forward_curve: &Arc<dyn InterestRateCurve + Send + Sync>,
    ) -> f64 {
        let incoming_id = *forward_curve.uuid();
        self.backend.get_or_compute(incoming_id, fixing_date, || {
            self.index.projected_rate(fixing_date, forward_curve)
        })
    }
}

// ── Type alias：對外只暴露這兩個名字 ─────────────────────────────────────────

pub type SingleThreadedCachedIndex = CachedInterestRateIndex<RefCellBackend>;
pub type MultiThreadedCachedIndex  = CachedInterestRateIndex<RwLockBackend>;