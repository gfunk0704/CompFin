use std::cmp::{
    max, 
    min
};
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::schedule::generationdirection::GenerationDirection;
use crate::time::schedule::scheduleperiod::CalculationPeriod;

#[derive(Clone, Copy, Deserialize)]
pub enum RelativeDateAlignment {
    StartDate,
    EndDate,
}

/// Optimized RelativeDateGenerator using enum dispatch instead of trait objects.
/// 
/// Benefits:
/// - Zero-cost abstraction (no dynamic dispatch overhead)
/// - Better compiler optimization
/// - Easier to serialize/deserialize
/// - Better error messages
#[derive(Clone, Deserialize)]
#[serde(tag = "type")]  // 加入這行
pub enum RelativeDateGenerator {
    ShiftDays(ShiftDaysConfig),
    FrequencyRatio(FrequencyRatioConfig),
}

#[derive(Clone, Deserialize)]
pub struct ShiftDaysConfig {
    alignment: RelativeDateAlignment,
    adjuster: BusinessDayAdjuster,
    days: i32,
}

#[derive(Clone, Deserialize)]
pub struct FrequencyRatioConfig {
    alignment: RelativeDateAlignment,
    adjuster: BusinessDayAdjuster,
    direction: GenerationDirection,
    every_n_period: usize,
    days: i32,
}

impl ShiftDaysConfig {
    pub fn new(
        alignment: RelativeDateAlignment,
        adjuster: BusinessDayAdjuster,
        days: i32,
    ) -> Self {
        ShiftDaysConfig {
            alignment,
            adjuster,
            days,
        }
    }

    pub fn alignment(&self) -> RelativeDateAlignment {
        self.alignment
    }

    pub fn adjuster(&self) -> &BusinessDayAdjuster {
        &self.adjuster
    }

    pub fn days(&self) -> i32 {
        self.days
    }
}

impl FrequencyRatioConfig {
    pub fn new(
        alignment: RelativeDateAlignment,
        adjuster: BusinessDayAdjuster,
        direction: GenerationDirection,
        every_n_period: usize,
        days: i32,
    ) -> Self {
        FrequencyRatioConfig {
            alignment,
            adjuster,
            direction,
            every_n_period,
            days,
        }
    }

    pub fn alignment(&self) -> RelativeDateAlignment {
        self.alignment
    }

    pub fn adjuster(&self) -> &BusinessDayAdjuster {
        &self.adjuster
    }

    pub fn direction(&self) -> GenerationDirection {
        self.direction
    }

    pub fn every_n_period(&self) -> usize {
        self.every_n_period
    }

    pub fn days(&self) -> i32 {
        self.days
    }
}

impl RelativeDateGenerator {
    /// Factory method for ShiftDays variant
    pub fn shift_days(
        alignment: RelativeDateAlignment,
        adjuster: BusinessDayAdjuster,
        days: i32,
    ) -> Self {
        RelativeDateGenerator::ShiftDays(ShiftDaysConfig::new(alignment, adjuster, days))
    }

    /// Factory method for FrequencyRatio variant
    pub fn frequency_ratio(
        alignment: RelativeDateAlignment,
        adjuster: BusinessDayAdjuster,
        direction: GenerationDirection,
        every_n_period: usize,
        days: i32,
    ) -> Self {
        RelativeDateGenerator::FrequencyRatio(FrequencyRatioConfig::new(
            alignment,
            adjuster,
            direction,
            every_n_period,
            days,
        ))
    }

    /// Generate dates based on calculation periods.
    /// 
    /// Optimizations:
    /// - Pre-allocates Vec with exact capacity
    /// - Uses resize instead of multiple push for repeated values
    /// - Minimizes calendar lookups
    pub fn generate(
        &self,
        calculation_periods: &[CalculationPeriod],
        calendar: &Arc<dyn HolidayCalendar>,
    ) -> Vec<NaiveDate> {
        match self {
            RelativeDateGenerator::ShiftDays(config) => {
                generate_shift_days(config, calculation_periods, calendar)
            }
            RelativeDateGenerator::FrequencyRatio(config) => {
                generate_frequency_ratio(config, calculation_periods, calendar)
            }
        }
    }
}

#[inline]
fn create_base_date_getter(
    alignment: RelativeDateAlignment,
) -> fn(&CalculationPeriod) -> NaiveDate {
    match alignment {
        RelativeDateAlignment::StartDate => |cp: &CalculationPeriod| cp.start_date(),
        RelativeDateAlignment::EndDate => |cp: &CalculationPeriod| cp.end_date(),
    }
}

/// Optimized shift days generation
fn generate_shift_days(
    config: &ShiftDaysConfig,
    calculation_periods: &[CalculationPeriod],
    calendar: &Arc<dyn HolidayCalendar>,
) -> Vec<NaiveDate> {
    let get_base_date = create_base_date_getter(config.alignment);
    
    // Pre-allocate with exact capacity
    let mut dates = Vec::with_capacity(calculation_periods.len());

    for period in calculation_periods {
        let unadjusted_date = get_base_date(period);
        let adjusted_date = if config.days != 0 {
            calendar.shift_n_business_day(unadjusted_date, config.days)
        } else {
            config.adjuster.adjust(unadjusted_date, calendar)
        };
        dates.push(adjusted_date);
    }

    dates
}

/// Optimized frequency ratio generation
fn generate_frequency_ratio(
    config: &FrequencyRatioConfig,
    calculation_periods: &[CalculationPeriod],
    calendar: &Arc<dyn HolidayCalendar>,
) -> Vec<NaiveDate> {
    if calculation_periods.is_empty() {
        return Vec::new();
    }

    let get_base_date = create_base_date_getter(config.alignment);
    
    // Pre-allocate with exact capacity
    let mut dates = Vec::with_capacity(calculation_periods.len());

    match config.direction {
        GenerationDirection::Forward => {
            generate_frequency_ratio_forward(
                config,
                calculation_periods,
                calendar,
                get_base_date,
                &mut dates,
            );
        }
        GenerationDirection::Backward => {
            generate_frequency_ratio_backward(
                config,
                calculation_periods,
                calendar,
                get_base_date,
                &mut dates,
            );
        }
    }

    dates
}

fn generate_frequency_ratio_forward(
    config: &FrequencyRatioConfig,
    calculation_periods: &[CalculationPeriod],
    calendar: &Arc<dyn HolidayCalendar>,
    get_base_date: fn(&CalculationPeriod) -> NaiveDate,
    dates: &mut Vec<NaiveDate>,
) {
    let get_index: fn(usize, usize) -> usize = match config.alignment {
        RelativeDateAlignment::StartDate => min,
        RelativeDateAlignment::EndDate => max,
    };

    let mut i = 0;
    while i < calculation_periods.len() {
        let n = (i + config.every_n_period).min(calculation_periods.len());
        let pos = get_index(i, n - 1);
        
        let unadjusted_date = get_base_date(&calculation_periods[pos]);
        let adjusted_date = if config.days > 0 {
            calendar.shift_n_business_day(unadjusted_date, config.days)
        } else {
            config.adjuster.adjust(unadjusted_date, calendar)
        };

        // Optimization: Use resize to fill multiple slots at once
        // instead of multiple push calls
        dates.resize(n, adjusted_date);
        
        // Alternatively, if resize doesn't work well:
        // dates.extend(std::iter::repeat(adjusted_date).take(n - i));
        
        i = n;
    }
}

fn generate_frequency_ratio_backward(
    config: &FrequencyRatioConfig,
    calculation_periods: &[CalculationPeriod],
    calendar: &Arc<dyn HolidayCalendar>,
    get_base_date: fn(&CalculationPeriod) -> NaiveDate,
    dates: &mut Vec<NaiveDate>,
) {
    let mut n = calculation_periods.len();
    
    while n > 0 {
        let i = if n >= config.every_n_period {
            n - config.every_n_period
        } else {
            0
        };
        
        let unadjusted_date = get_base_date(&calculation_periods[n - 1]);
        let adjusted_date = if config.days > 0 {
            calendar.shift_n_business_day(unadjusted_date, config.days)
        } else {
            config.adjuster.adjust(unadjusted_date, calendar)
        };

        // Fill from i to n with the same date
        let count = n - i;
        dates.extend(std::iter::repeat(adjusted_date).take(count));
        
        n = i;
    }
    
    // Reverse at the end (single operation)
    dates.reverse();
}