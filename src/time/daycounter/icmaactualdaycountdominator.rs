use std::cmp::Ordering;
use std::sync::Arc;

use chrono::NaiveDate;

use super::daycounter::{
    DayCounterDominator, DayCounterDominatorGenerator, DayCounterGenerationError,
    DayCounterNumerator,
};
use super::super::period::TimeUnit;
use super::super::schedule::scheduleperiod::CalculationPeriod;
use super::super::schedule::schedule::Schedule;

// ─────────────────────────────────────────────────────────────────────────────
// ICMAActualDayCounterDominator
// ─────────────────────────────────────────────────────────────────────────────
//
// # 變更說明：移除 generate_extension_periods
//
// 原始設計：當 schedule 有 stub 時，呼叫 `generate_extension_periods` 重新產生
// 一組「延伸超過 maturity」的 quasi-periods，以便：
//   1. Binary search 能找到 stub 期間的任意日期落在哪個 quasi-period
//   2. 用 quasi-period 的自然長度（而非截短後的 stub 長度）作為 ICMA 分母
//
// 現在 CalculationPeriod 已攜帶 regular_start_date / regular_end_date，
// 兩個目的都可以直接從 schedule periods 滿足：
//
//   1. Binary search 用 actual start/end date（覆蓋所有實際查詢日期）
//   2. period_length = regular_end_date - regular_start_date（自然 tenor 長度）
//
// 因此 generate_extension_periods 與 quasi_periods 欄位均不再需要。

pub struct ICMAActualDayCounterDominator {
    /// 實際的 calculation periods（含 stub 的 actual start/end），
    /// 用於 binary search 定位查詢日期。
    periods: Vec<CalculationPeriod>,
    /// 每個 period 的「自然 tenor 長度」（天數），作為 ICMA 分母。
    /// stub period 使用 regular_end - regular_start，而非 end - start。
    period_lengths: Vec<f64>,
    last_index: usize,
    last_period_end: NaiveDate,
    coupon_frequency: f64,
}

impl ICMAActualDayCounterDominator {
    pub fn new(schedule: &Schedule) -> Result<Self, DayCounterGenerationError> {
        let frequency_period = schedule
            .generator()
            .calculation_period_generator()
            .frequency();

        let coupon_frequency = match frequency_period.unit() {
            TimeUnit::Years => {
                if frequency_period.number() == 1 {
                    1.0
                } else {
                    return Err(DayCounterGenerationError::IrregularFrequencyForICMADominator);
                }
            }
            TimeUnit::Months => {
                if frequency_period.number() % 12 == 0 {
                    (frequency_period.number() / 12) as f64
                } else {
                    12.0 / frequency_period.number() as f64
                }
            }
            _ => return Err(DayCounterGenerationError::IrregularFrequencyForICMADominator),
        };

        let periods: Vec<CalculationPeriod> = schedule
            .schedule_periods()
            .iter()
            .map(|sp| sp.calculation_period())
            .collect();

        if periods.is_empty() {
            return Err(DayCounterGenerationError::ScheduleNotGiven);
        }

        // period_length 用 regular_end - regular_start：
        //   - 正常 period：regular == actual，結果與舊版相同
        //   - Stub period：用自然 tenor 長度作為 ICMA 分母（正確語意）
        let period_lengths: Vec<f64> = periods
            .iter()
            .map(|p| {
                (p.regular_end_date() - p.regular_start_date()).num_days() as f64
            })
            .collect();

        let last_index = periods.len() - 1;
        let last_period_end = periods[last_index].end_date();

        Ok(Self {
            periods,
            period_lengths,
            last_index,
            last_period_end,
            coupon_frequency,
        })
    }
}

impl DayCounterDominator for ICMAActualDayCounterDominator {
    fn year_fraction(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        numerator: &Arc<dyn DayCounterNumerator>,
    ) -> f64 {
        assert!(start_date >= self.periods[0].start_date());
        assert!(end_date <= self.last_period_end);

        // Binary search 使用 actual end_date（包含 stub 的實際結束日）。
        // 不再需要延伸超過 maturity 的 quasi-periods。
        let start_idx = self
            .periods
            .binary_search_by(|p| match start_date.cmp(&p.end_date()) {
                Ordering::Equal => Ordering::Less,
                ord => ord,
            })
            .unwrap_err();

        let end_idx = if end_date < self.last_period_end {
            self.periods
                .binary_search_by(|p| match end_date.cmp(&p.end_date()) {
                    Ordering::Equal => Ordering::Less,
                    ord => ord,
                })
                .unwrap_err()
        } else {
            self.last_index
        };

        if start_idx == end_idx {
            // 同一個 period 內：actual days / regular period length
            numerator.days_between(start_date, end_date)
                / self.period_lengths[start_idx]
        } else {
            let p_start = self.periods[start_idx];
            let start_fraction = numerator.days_between(start_date, p_start.end_date())
                / self.period_lengths[start_idx];

            let p_end = self.periods[end_idx];
            let end_fraction = numerator.days_between(p_end.start_date(), end_date)
                / self.period_lengths[end_idx];

            (start_fraction
                + end_fraction
                + (end_idx - start_idx - 1) as f64)
                / self.coupon_frequency
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ICMADayCounterDominatorGenerator
// ─────────────────────────────────────────────────────────────────────────────

pub struct ICMADayCounterDominatorGenerator;

impl ICMADayCounterDominatorGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl DayCounterDominatorGenerator for ICMADayCounterDominatorGenerator {
    fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<Arc<dyn DayCounterDominator>, DayCounterGenerationError> {
        match schedule_opt {
            None => Err(DayCounterGenerationError::ScheduleNotGiven),
            Some(schedule) => {
                let dominator = ICMAActualDayCounterDominator::new(schedule)?;
                Ok(Arc::new(dominator))
            }
        }
    }
}
