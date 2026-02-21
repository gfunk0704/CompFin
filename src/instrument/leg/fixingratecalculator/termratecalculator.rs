// ── termratecalculator.rs ────────────────────────────────────────────────────
//
// TermRateCalculator 負責：
//   1. 從 schedule 取出每個 period 的 CalculationPeriod
//   2. 判斷是否為 stub，依 StubRateConvention 決定如何查詢 past fixing
//   3. 呼叫 TermRateIndex::fixing_rate_for_period，傳入「正確的」CalculationPeriod
//
// # StubRateConvention 放在這裡的理由
//
// stub convention 是契約層的決定，同一個 TermRateIndex（例如 3M LIBOR）
// 在不同 IRS 合約中可以有不同的 stub 處理方式。
// TermRateIndex 本身只知道「給定任意 period，回傳 fixing 或 projected rate」，
// 不應該感知契約層的 stub convention。
//
// 各 convention 的實作邏輯：
//
//   Straight：傳入 regular period（period.regular_start ~ regular_end）
//     → index 查出自然 tenor 的 fixing，不做縮放
//
//   Interpolation：分別傳入 short/long period，各自查 fixing，再線性插值
//     → 插值公式：short + (long - short) × (stub_tau - short_tau) / (long_tau - short_tau)
//
//   Proportional：傳入 regular period 查 fixing，再乘以 stub/regular 比例
//     → rate = regular_rate × (stub_tau / regular_tau)
//
// Projection 路徑不需要任何 stub 特殊處理：
//   forward curve 是連續的，D(start)/D(end) 對任意區間都正確。
//   TermRateIndex::projected_rate_for_period 直接用 actual start/end，
//   stub_rate_convention 完全不影響 projection。

use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Deserialize;

use super::fixingratecalculator::{FixingRateCalculator, FixingRateCalculatorGenerator};
use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::period::Period;
use crate::time::schedule::schedule::Schedule;
use crate::time::schedule::scheduleperiod::CalculationPeriod;


// ─────────────────────────────────────────────────────────────────────────────
// StubRateConvention
// ─────────────────────────────────────────────────────────────────────────────

/// Term rate index 在 stub period 的 past fixing 計算慣例。
///
/// Projection 下不需要此 convention：forward curve 連續，
/// `D(stub_start)/D(stub_end)` 對任意區間都正確。
///
/// 慣例差異只出現在 past fixing：stub 長度的歷史 fixing 通常不存在，
/// 需要從已知 tenor 推算。
///
/// 參考：ISDA 2006 Definitions, CS Lucas user guide
#[derive(Clone, Copy, Deserialize)]
pub enum StubRateConvention {
    /// 直接使用 regular period（自然 tenor）的 fixing，不做縮放。
    ///
    /// 例：short 6M stub 在 1Y index 下，直接使用 1Y fixing。
    /// 操作最簡單，但有時間錯配。
    Straight,

    /// 在兩個鄰近 tenor 之間線性插值。
    ///
    /// 例：40-day stub 在 1M/3M 兩個 fixings 之間插值。
    /// short_tenor / long_tenor 由合約指定，index 的 tenor 不影響插值端點。
    Interpolation {
        short_tenor: Period,
        long_tenor: Period,
    },

    /// 按年分數比例縮放：`regular_rate × (stub_tau / regular_tau)`。
    ///
    /// 例：40-day stub 對應 3M（91-day）：`rate × (40/91)`。
    /// 適用於 short stub 且差距不大的場景。
    Proportional,
}

impl Default for StubRateConvention {
    fn default() -> Self {
        StubRateConvention::Straight
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// TermRateCalculator
// ─────────────────────────────────────────────────────────────────────────────

pub struct TermRateCalculator {
    index: Arc<dyn InterestRateIndex + Send + Sync>,
    periods: Vec<CalculationPeriod>,
    stub_rate_convention: StubRateConvention,
}

impl TermRateCalculator {
    pub fn new(
        index: Arc<dyn InterestRateIndex + Send + Sync>,
        schedule: &Schedule,
        stub_rate_convention: StubRateConvention,
    ) -> Self {
        let periods = schedule
            .schedule_periods()
            .iter()
            .map(|sp| sp.calculation_period())
            .collect();

        Self { index, periods, stub_rate_convention }
    }

    // ── stub past fixing 計算 ─────────────────────────────────────────────

    /// Straight：傳 regular period 給 index，直接查自然 tenor 的 fixing。
    fn stub_past_straight(
        &self,
        period: &CalculationPeriod,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let regular = CalculationPeriod::regular(
            period.regular_start_date(),
            period.regular_end_date(),
        );
        self.index.fixing_rate_for_period(&regular, None, pricing_condition)
    }

    /// Interpolation：short/long period 各自查 fixing，再線性插值。
    fn stub_past_interpolated(
        &self,
        period: &CalculationPeriod,
        short_tenor: Period,
        long_tenor: Period,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let adjuster = self.index.adjuster();
        let calendar = self.index.calendar();
        let dc       = self.index.day_counter();
        let start    = period.start_date();

        let short_end = adjuster.from_tenor_to_date(start, short_tenor, calendar);
        let long_end  = adjuster.from_tenor_to_date(start, long_tenor,  calendar);

        let short_period = CalculationPeriod::regular(start, short_end);
        let long_period  = CalculationPeriod::regular(start, long_end);

        let short_rate = self.index.fixing_rate_for_period(&short_period, None, pricing_condition)?;
        let long_rate  = self.index.fixing_rate_for_period(&long_period,  None, pricing_condition)?;

        let stub_tau  = dc.year_fraction(start, period.end_date());
        let short_tau = dc.year_fraction(start, short_end);
        let long_tau  = dc.year_fraction(start, long_end);

        if (long_tau - short_tau).abs() < 1e-10 {
            return Some(short_rate);
        }

        let weight = (stub_tau - short_tau) / (long_tau - short_tau);
        Some(short_rate + weight * (long_rate - short_rate))
    }

    /// Proportional：查 regular rate，乘以 stub/regular 年分數比。
    fn stub_past_proportional(
        &self,
        period: &CalculationPeriod,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        let dc      = self.index.day_counter();
        let regular = CalculationPeriod::regular(
            period.regular_start_date(),
            period.regular_end_date(),
        );

        let regular_rate = self.index.fixing_rate_for_period(&regular, None, pricing_condition)?;
        let stub_tau     = dc.year_fraction(period.start_date(),         period.end_date());
        let regular_tau  = dc.year_fraction(period.regular_start_date(), period.regular_end_date());

        if regular_tau.abs() < 1e-10 {
            return Some(regular_rate);
        }

        Some(regular_rate * stub_tau / regular_tau)
    }

    /// stub past fixing 的入口，依 stub_rate_convention 分派。
    fn stub_past_fixing(
        &self,
        period: &CalculationPeriod,
        pricing_condition: &PricingCondition,
    ) -> Option<f64> {
        match self.stub_rate_convention {
            StubRateConvention::Straight => {
                self.stub_past_straight(period, pricing_condition)
            }
            StubRateConvention::Interpolation { short_tenor, long_tenor } => {
                self.stub_past_interpolated(period, short_tenor, long_tenor, pricing_condition)
            }
            StubRateConvention::Proportional => {
                self.stub_past_proportional(period, pricing_condition)
            }
        }
    }
}

impl FixingRateCalculator for TermRateCalculator {
    fn index(&self) -> &Arc<dyn InterestRateIndex + Send + Sync> {
        &self.index
    }

    fn relative_dates(&self, i: usize) -> HashSet<NaiveDate> {
        self.index.relative_dates_for_period(&self.periods[i])
    }

    fn fixing(
        &self,
        i: usize,
        forward_curve: &Arc<dyn InterestRateCurve>,
        pricing_condition: &PricingCondition,
    ) -> f64 {
        let period = &self.periods[i];

        let is_past = period.start_date() < *pricing_condition.horizon()
            || (period.start_date() == *pricing_condition.horizon()
                && !pricing_condition.estimate_horizon_index());

        if is_past && period.is_stub() {
            // Stub past：依 convention 計算
            self.stub_past_fixing(period, pricing_condition).unwrap_or(0.0)
        } else {
            // 正常 past 或所有 projection：直接委託給 index
            self.index
                .fixing_rate_for_period(period, Some(forward_curve), pricing_condition)
                .unwrap_or(0.0)
        }
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// TermRateCalculatorGenerator
// ─────────────────────────────────────────────────────────────────────────────

pub struct TermRateCalculatorGenerator {
    index: Arc<dyn InterestRateIndex + Send + Sync>,
    stub_rate_convention: StubRateConvention,
}

impl TermRateCalculatorGenerator {
    pub fn new(
        index: Arc<dyn InterestRateIndex + Send + Sync>,
        stub_rate_convention: StubRateConvention,
    ) -> Self {
        Self { index, stub_rate_convention }
    }

    /// 使用預設的 Straight convention。
    pub fn new_straight(index: Arc<dyn InterestRateIndex + Send + Sync>) -> Self {
        Self::new(index, StubRateConvention::Straight)
    }
}

impl FixingRateCalculatorGenerator for TermRateCalculatorGenerator {
    fn index(&self) -> &Arc<dyn InterestRateIndex + Send + Sync> {
        &self.index
    }

    fn generate(&self, schedule: &Schedule) -> Arc<dyn FixingRateCalculator> {
        Arc::new(TermRateCalculator::new(
            self.index.clone(),
            schedule,
            self.stub_rate_convention,
        ))
    }
}
