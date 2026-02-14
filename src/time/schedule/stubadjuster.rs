use chrono::NaiveDate;
use serde::Deserialize;

use super::scheduleperiod::CalculationPeriod;



fn extend(_forward: bool, 
          _last_date: NaiveDate, 
          _get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
          calculation_periods: Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    calculation_periods
}

fn remove(forward: bool, 
          last_date: NaiveDate, 
          get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
          calculation_periods: Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    let has_stub: bool = get_period_last_date(&calculation_periods) != last_date;
    if has_stub {
        if forward {
            calculation_periods[..(calculation_periods.len() - 1)].to_vec()
        } else {
            calculation_periods[1..].to_vec()
        }
    } else {
        calculation_periods
    }
}

fn retain_last(last_date: NaiveDate, calculation_periods: &Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    let last_period = calculation_periods.last().unwrap();
    let mut adjusted_periods = calculation_periods[..(calculation_periods.len() - 1)].to_vec();
    adjusted_periods.push(CalculationPeriod::new(last_period.start_date(), last_date));
    adjusted_periods
}

fn retain_first(last_date: NaiveDate, calculation_periods: &Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    let first_period = calculation_periods.first().unwrap();
    let mut adjusted_periods = calculation_periods[1..].to_vec();
    adjusted_periods.insert(0, CalculationPeriod::new(last_date, first_period.end_date()));
    adjusted_periods
}

fn retain(forward: bool, 
          last_date: NaiveDate, 
          get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate, 
          calculation_periods: Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    let has_stub: bool = get_period_last_date(&calculation_periods) != last_date;
    if has_stub {
        if forward {
            retain_last(last_date, &calculation_periods)
        } else {
            retain_first(last_date, &calculation_periods)
        }
    } else {
        calculation_periods
    }
}

fn combine_last(last_date: NaiveDate, calculation_periods: &Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    let last = calculation_periods.len() - 1;
    let before_last = last - 1;
    let mut adjusted_periods = calculation_periods[..before_last].to_vec();
    let combined_period = CalculationPeriod::new(calculation_periods[before_last].start_date(), last_date);
    adjusted_periods.push(combined_period);
    adjusted_periods
}

fn combine_first(last_date: NaiveDate, calculation_periods: &Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    let mut adjusted_periods = calculation_periods[2..].to_vec();
    let combined_period = CalculationPeriod::new(last_date, calculation_periods[1].end_date());
    adjusted_periods.insert(0, combined_period);
    adjusted_periods
}

fn combine(forward: bool, 
           last_date: NaiveDate, 
           get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate, 
           calculation_periods: Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    if calculation_periods.len() == 1 {
        return retain(forward, last_date, get_period_last_date, calculation_periods)
    } 
    
    let has_stub: bool = get_period_last_date(&calculation_periods) != last_date;
    if has_stub {
        if forward {
            combine_last(last_date, &calculation_periods)
        } else {
            combine_first(last_date, &calculation_periods)
        }
    } else {
        calculation_periods
    }
}

fn smart_combine(forward: bool, 
                 last_date: NaiveDate, 
                 get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate, 
                 calculation_periods: Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
    if calculation_periods.len() == 1 {
        return  retain(forward, last_date, get_period_last_date, calculation_periods)
    } 
    
    if get_period_last_date(&calculation_periods) != last_date {
        if forward {
            let last_period = calculation_periods.last().unwrap();
            if (last_date - last_period.start_date()).num_days() < 7 {
                combine_last(last_date, &calculation_periods)
            } else {
                retain_last(last_date, &calculation_periods)
            }
        } else {
            let first_period = calculation_periods.first().unwrap();
            if (first_period.end_date() - last_date).num_days() < 7 {
                combine_first(last_date, &calculation_periods)
            } else {
                retain_first(last_date, &calculation_periods)
            }
        }
    } else {
        calculation_periods
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Deserialize)]
pub enum StubConvention {
    Extend,
    Reomve,
    Retain,
    Combine,
    SmartCombine
}

#[derive(Clone, Copy)]
pub struct StubAdjuster {
    convention: StubConvention,
    forward: bool,
    get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
    adjust_impl: fn(bool, NaiveDate, fn(&Vec<CalculationPeriod>) -> NaiveDate, Vec<CalculationPeriod>) -> Vec<CalculationPeriod>
}


impl StubAdjuster {
    pub fn new(convention: StubConvention, forward: bool) -> StubAdjuster {
        let adjust_impl = match  convention {
            StubConvention::Combine => combine,
            StubConvention::Extend => extend,
            StubConvention::Reomve => remove,
            StubConvention::Retain => retain,
            StubConvention::SmartCombine => smart_combine
        };

        let get_period_last_date = if forward {
            | calculation_periods: &Vec<CalculationPeriod> | calculation_periods.last().unwrap().end_date()
        } else {
            | calculation_periods: &Vec<CalculationPeriod> | calculation_periods.first().unwrap().start_date()
        };

        StubAdjuster { convention: convention, forward: forward, get_period_last_date: get_period_last_date, adjust_impl: adjust_impl }
    }

    pub fn convention(&self) -> StubConvention {
        self.convention
    }

    pub fn forward(&self) -> bool {
        self.forward
    }

    pub fn adjust(&self, last_date: NaiveDate, calculation_periods: Vec<CalculationPeriod>) -> Vec<CalculationPeriod> {
        (self.adjust_impl)(self.forward, last_date, self.get_period_last_date, calculation_periods)
    }
 }

