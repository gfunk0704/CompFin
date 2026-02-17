use std::cmp::Ordering;
use std::sync::Arc; // 變更：Rc → Arc

use chrono::NaiveDate;

use super::daycounter::{
    DayCounterDominator, DayCounterDominatorGenerator, DayCounterGenerationError,
    DayCounterNumerator,
};
use super::super::period::TimeUnit;
use super::super::schedule::scheduleperiod::CalculationPeriod;
use super::super::schedule::schedule::Schedule;
use super::super::schedule::stubadjuster::StubConvention;

pub struct ICMAActualDayCounterDominator {
    quasi_periods: Vec<CalculationPeriod>,
    period_lengths: Vec<f64>,
    last_period_end: NaiveDate,
    last_index: usize,
    coupon_frequency: f64,
}

impl ICMAActualDayCounterDominator {
    pub fn new(
        schedule: &Schedule,
    ) -> Result<ICMAActualDayCounterDominator, DayCounterGenerationError> {
        let stub_convention = schedule
            .generator()
            .calculation_period_generator()
            .stub_convention();

        let quasi_periods: Vec<CalculationPeriod> = match stub_convention {
            StubConvention::Extend => {
                schedule
                    .schedule_periods()
                    .iter()
                    .map(|sp| sp.calculation_period())
                    .collect()
            }
            _ => schedule
                .generator()
                .calculation_period_generator()
                .generate_extension_periods(
                    schedule.calendar(),
                    schedule.horizon(),
                    schedule.maturity(),
                )
                .unwrap(),
        };

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
                if (frequency_period.number() % 12) == 0 {
                    (frequency_period.number() / 12) as f64
                } else {
                    return Err(DayCounterGenerationError::IrregularFrequencyForICMADominator);
                }
            }
            _ => {
                return Err(DayCounterGenerationError::IrregularFrequencyForICMADominator);
            }
        };

        let period_lengths: Vec<f64> = quasi_periods
            .iter()
            .map(|p| (p.end_date() - p.start_date()).num_days() as f64)
            .collect();

        let last_index = quasi_periods.len() - 1;
        let last_period_end = quasi_periods[last_index].end_date();

        Ok(ICMAActualDayCounterDominator {
            quasi_periods,
            period_lengths,
            last_period_end,
            last_index,
            coupon_frequency,
        })
    }
}

impl DayCounterDominator for ICMAActualDayCounterDominator {
    fn year_fraction(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        numerator: &Arc<dyn DayCounterNumerator>, // 變更：Rc → Arc
    ) -> f64 {
        assert!(start_date >= self.quasi_periods[0].start_date());
        assert!(end_date <= self.last_period_end);

        let start_date_index = self
            .quasi_periods
            .binary_search_by(|p| match start_date.cmp(&p.end_date()) {
                Ordering::Equal => Ordering::Less,
                ord => ord,
            })
            .unwrap_err();

        let end_date_index = if end_date < self.last_period_end {
            self.quasi_periods
                .binary_search_by(|p| match end_date.cmp(&p.end_date()) {
                    Ordering::Equal => Ordering::Less,
                    ord => ord,
                })
                .unwrap_err()
        } else {
            self.last_index
        };

        if start_date_index == end_date_index {
            numerator.days_between(start_date, end_date)
                / self.period_lengths[start_date_index]
        } else {
            let p1 = self.quasi_periods[start_date_index];
            let start_fraction = numerator.days_between(start_date, p1.end_date())
                / self.period_lengths[start_date_index];

            let p2 = self.quasi_periods[end_date_index];
            let end_fraction = numerator.days_between(p2.start_date(), end_date)
                / self.period_lengths[end_date_index];

            (start_fraction
                + end_fraction
                + ((end_date_index - start_date_index - 1) as f64))
                / self.coupon_frequency
        }
    }
}

pub struct ICMADayCounterDominatorGenerator;

impl ICMADayCounterDominatorGenerator {
    pub fn new() -> ICMADayCounterDominatorGenerator {
        ICMADayCounterDominatorGenerator {}
    }
}

impl DayCounterDominatorGenerator for ICMADayCounterDominatorGenerator {
    fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<Arc<dyn DayCounterDominator>, DayCounterGenerationError> { // 變更：Rc → Arc
        if schedule_opt.is_none() {
            Err(DayCounterGenerationError::ScheduleNotGiven)
        } else {
            let dominator = ICMAActualDayCounterDominator::new(schedule_opt.unwrap())?;
            Ok(Arc::new(dominator))
        }
    }
}
