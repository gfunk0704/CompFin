use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounter;
use crate::time::period::Period;
use crate::time::schedule::scheduleperiod::CalculationPeriod;


#[derive(PartialEq, Eq, Deserialize)]
pub enum InterestRateIndexType {
    TermRate,
    CompoundingRate,
}

pub trait InterestRateIndex {

    // ── Index 靜態屬性 ────────────────────────────────────────────────────

    fn adjuster(&self) -> &BusinessDayAdjuster;

    fn calendar(&self) -> &Arc<dyn HolidayCalendar>;

    fn start_lag(&self) -> u32;

    /// Index 的自然 tenor（例如 3M LIBOR、1Y SOFR compounding）。
    ///
    /// 用於直接查詢 index rate 的場景（非透過 leg）。
    /// Leg 傳入 `CalculationPeriod` 時，不使用此 tenor。
    fn tenor(&self) -> &Period;

    fn day_counter(&self) -> &DayCounter;

    fn index_type(&self) -> InterestRateIndexType;

    fn reference_curve_name(&self) -> &String;

    fn past_fixings(&self) -> &HashMap<NaiveDate, f64>;

    // ── 由 fixing_date 推算日期 ───────────────────────────────────────────

    fn start_date(&self, fixing_date: NaiveDate) -> NaiveDate;

    fn end_date(&self, fixing_date: NaiveDate) -> NaiveDate;

    // ── 核心計算：接受 CalculationPeriod ─────────────────────────────────
    //
    // Leg 直接傳入 schedule 產生的 CalculationPeriod，
    // 不需要經過 fixing_date → start/end 的轉換。
    //
    // CalculationPeriod 同時攜帶：
    //   period.start_date()        / period.end_date()         ← 實際計算區間
    //   period.regular_start_date()/ period.regular_end_date() ← 自然 tenor 的完整範圍
    //   period.is_stub()                                        ← 是否為 stub
    //
    // TermRateIndex 可利用 is_stub() 切換 straight / interpolation / proportional 慣例。
    // CompoundingRateIndex 逐日計算，stub 長度自然被處理，不需要 is_stub()。

    /// 計算 CalculationPeriod 的 projected rate（pure forward，不使用 past fixings）。
    ///
    /// - `TermRateIndex`：compounding.implied_rate(D(start)/D(end), tau)
    ///   stub 時依 stub convention 使用實際 start/end，與 regular 無關（curve 本身連續）
    /// - `CompoundingRateIndex`：∏ overnight DFs，逐日累積
    fn projected_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64;

    /// 計算 CalculationPeriod 需要的所有 discount factor 日期。
    ///
    /// 供 `PrecomputedDiscountCurve` 預先 warm-up cache 使用。
    /// - `TermRateIndex`：{start_date, end_date}（2 個日期）
    /// - `CompoundingRateIndex`：區間內所有業務日 + end_date（N + 1 個日期）
    fn relative_dates_for_period(
        &self,
        period: &CalculationPeriod,
    ) -> HashSet<NaiveDate>;

    /// 混合計算：past fixings 用實際值，future 用 projection。
    ///
    /// - `TermRateIndex`：
    ///   - 非 stub：以 start_date 判斷整個 period 是 past 或 future
    ///   - Stub：依 stub_rate_convention 選擇 straight / interpolation / proportional
    /// - `CompoundingRateIndex`：覆寫此方法，逐日判斷 past/future
    fn fixing_rate_for_period(
        &self,
        period: &CalculationPeriod,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64>;

    // ── Default 實作：fixing_date 版本（委託給 _for_period）─────────────
    //
    // 保留 fixing_date 介面供直接查詢 index rate 使用（不透過 leg）。
    // 實作方不需要覆寫這三個方法。

    fn projected_rate(
        &self,
        fixing_date: NaiveDate,
        forward_curve: &Arc<dyn InterestRateCurve>,
    ) -> f64 {
        let period = CalculationPeriod::regular(
            self.start_date(fixing_date),
            self.end_date(fixing_date),
        );
        self.projected_rate_for_period(&period, forward_curve)
    }

    fn relative_dates(&self, fixing_date: NaiveDate) -> HashSet<NaiveDate> {
        let period = CalculationPeriod::regular(
            self.start_date(fixing_date),
            self.end_date(fixing_date),
        );
        self.relative_dates_for_period(&period)
    }

    fn fixing_rate(
        &self,
        fixing_date: NaiveDate,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let period = CalculationPeriod::regular(
            self.start_date(fixing_date),
            self.end_date(fixing_date),
        );
        self.fixing_rate_for_period(&period, forward_curve_opt, pricing_condition)
    }
}
