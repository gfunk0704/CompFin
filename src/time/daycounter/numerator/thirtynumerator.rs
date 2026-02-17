use std::sync::Arc; // 變更：Rc → Arc

use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

use crate::time::daycounter::daycounter::{
    DayCounterNumerator,
    DayCounterNumeratorGenerator,
    DayCounterGenerationError
};
use crate::time::schedule::schedule::Schedule;
use crate::time::utility::days_of_month;

#[derive(Clone, Copy)]
struct YMD {
    y: i32,
    m: i32,
    d: i32
}

impl YMD {
    pub fn from_naive_date(date: NaiveDate) -> YMD {
        YMD { y: date.year(), m: date.month() as i32, d: date.day() as i32 }
    }
}

impl PartialEq<YMD> for YMD {
    fn eq(&self, other: &YMD) -> bool {
        (self.y == other.y) && (self.m == other.m) && (self.d == other.d)
    }
}

#[derive(Clone, Copy, Deserialize)]
pub enum ThirtyAdjstmentCondition {
    None,
    GreaterThanThirty,
    GreaterThanOrEqualtToThirty,
    IsLastDayOfMonth,
    IsNoLeapLastDayOfMonth,
    LastDayUnlessFebButTermination
}

fn none_condition(_ymd: YMD, _termination_ymd: YMD) -> bool { true }
fn is_greater_than_thirty(ymd: YMD, _termination_ymd: YMD) -> bool { ymd.d > 30 }
fn is_greater_than_or_equal_tothirty(ymd: YMD, _termination_ymd: YMD) -> bool { ymd.d >= 30 }
fn is_last_day_of_month(ymd: YMD, _termination_ymd: YMD) -> bool {
    ymd.d == (days_of_month(ymd.y, ymd.m as u32) as i32)
}
fn is_no_leap_last_day_of_month(ymd: YMD, termination_ymd: YMD) -> bool {
    if ymd.m == 2 { ymd.d == 28 } else { is_last_day_of_month(ymd, termination_ymd) }
}
fn is_last_day_unless_feb_but_termination(ymd: YMD, termination_ymd: YMD) -> bool {
    if (ymd.m == 2) && (ymd == termination_ymd) {
        false
    } else {
        is_last_day_of_month(ymd, termination_ymd)
    }
}

fn get_adjustment_condition_impl(adjustment_condition: &ThirtyAdjstmentCondition) -> fn(YMD, YMD) -> bool {
    match adjustment_condition {
        ThirtyAdjstmentCondition::None => none_condition,
        ThirtyAdjstmentCondition::GreaterThanThirty => is_greater_than_thirty,
        ThirtyAdjstmentCondition::GreaterThanOrEqualtToThirty => is_greater_than_or_equal_tothirty,
        ThirtyAdjstmentCondition::IsLastDayOfMonth => is_last_day_of_month,
        ThirtyAdjstmentCondition::IsNoLeapLastDayOfMonth => is_no_leap_last_day_of_month,
        ThirtyAdjstmentCondition::LastDayUnlessFebButTermination => is_last_day_unless_feb_but_termination
    }
}

#[derive(Clone, Copy, Deserialize)]
pub enum ThirtyAdjustment {
    ToThirty,
    ToNextMonthFirst
}

fn to_next_month_first(ymd: YMD) -> YMD {
    if ymd.m < 12 { YMD { y: ymd.y, m: ymd.m + 1, d: ymd.d } }
    else           { YMD { y: ymd.y + 1, m: 1, d: ymd.d } }
}

fn to_thirty(ymd: YMD) -> YMD {
    YMD { y: ymd.y, m: ymd.m + 1, d: 30 }
}

fn get_adjustment_impl(adjustment: &ThirtyAdjustment) -> fn(YMD) -> YMD {
    match adjustment {
        ThirtyAdjustment::ToNextMonthFirst => to_next_month_first,
        ThirtyAdjustment::ToThirty => to_thirty
    }
}

pub struct ThirtyNumerator {
    start_date_condition: ThirtyAdjstmentCondition,
    start_date_condition_impl: fn(YMD, YMD) -> bool,
    start_date_adjustment: ThirtyAdjustment,
    start_date_adjustment_impl: fn(YMD) -> YMD,
    end_date_condition: ThirtyAdjstmentCondition,
    end_date_condition_impl: fn(YMD, YMD) -> bool,
    additional_start_date_condition: ThirtyAdjstmentCondition,
    additional_start_date_condition_impl: fn(YMD, YMD) -> bool,
    end_date_adjustment: ThirtyAdjustment,
    end_date_adjustment_impl: fn(YMD) -> YMD,
    termination_ymd: YMD
}

impl ThirtyNumerator {
    pub fn new(
        start_date_condition: ThirtyAdjstmentCondition,
        start_date_adjustment: ThirtyAdjustment,
        end_date_condition: ThirtyAdjstmentCondition,
        additional_start_date_condition: ThirtyAdjstmentCondition,
        end_date_adjustment: ThirtyAdjustment,
        schedule: &Schedule
    ) -> ThirtyNumerator {
        ThirtyNumerator {
            start_date_condition_impl: get_adjustment_condition_impl(&start_date_condition),
            start_date_condition,
            start_date_adjustment_impl: get_adjustment_impl(&start_date_adjustment),
            start_date_adjustment,
            end_date_condition_impl: get_adjustment_condition_impl(&end_date_condition),
            end_date_condition,
            additional_start_date_condition_impl: get_adjustment_condition_impl(&additional_start_date_condition),
            additional_start_date_condition,
            end_date_adjustment_impl: get_adjustment_impl(&end_date_adjustment),
            end_date_adjustment,
            termination_ymd: YMD::from_naive_date(
                schedule.schedule_periods().last().unwrap().calculation_period().end_date()
            )
        }
    }

    pub fn start_date_condition(&self) -> ThirtyAdjstmentCondition { self.start_date_condition }
    pub fn start_date_adjustment(&self) -> ThirtyAdjustment { self.start_date_adjustment }
    pub fn end_date_condition(&self) -> ThirtyAdjstmentCondition { self.end_date_condition }
    pub fn additional_start_date_condition(&self) -> ThirtyAdjstmentCondition { self.additional_start_date_condition }
    pub fn end_date_adjustment(&self) -> ThirtyAdjustment { self.end_date_adjustment }
    pub fn termination_date(&self) -> NaiveDate {
        NaiveDate::from_ymd_opt(
            self.termination_ymd.y,
            self.termination_ymd.m as u32,
            self.termination_ymd.d as u32
        ).unwrap()
    }
}

impl DayCounterNumerator for ThirtyNumerator {
    fn days_between(&self, d1: NaiveDate, d2: NaiveDate) -> f64 {
        let mut ymd1 = YMD::from_naive_date(d1);
        let mut ymd2 = YMD::from_naive_date(d2);
        if (self.start_date_condition_impl)(ymd1, self.termination_ymd) {
            ymd1 = (self.start_date_adjustment_impl)(ymd1);
        }
        if (self.additional_start_date_condition_impl)(ymd1, self.termination_ymd) &&
           (self.end_date_condition_impl)(ymd2, self.termination_ymd) {
            ymd2 = (self.end_date_adjustment_impl)(ymd2);
        }
        (360 * (ymd2.y - ymd1.y) + 30 * (ymd2.m - ymd1.m) + (ymd2.d - ymd1.d)) as f64
    }
}

#[derive(Deserialize)]
pub struct ThirtyNumeratorGenerator {
    start_date_condition: ThirtyAdjstmentCondition,
    start_date_adjustment: ThirtyAdjustment,
    end_date_condition: ThirtyAdjstmentCondition,
    additional_start_date_condition: ThirtyAdjstmentCondition,
    end_date_adjustment: ThirtyAdjustment
}

impl ThirtyNumeratorGenerator {
    pub fn new(
        start_date_condition: ThirtyAdjstmentCondition,
        start_date_adjustment: ThirtyAdjustment,
        end_date_condition: ThirtyAdjstmentCondition,
        additional_start_date_condition: ThirtyAdjstmentCondition,
        end_date_adjustment: ThirtyAdjustment
    ) -> ThirtyNumeratorGenerator {
        ThirtyNumeratorGenerator {
            start_date_condition,
            start_date_adjustment,
            end_date_condition,
            additional_start_date_condition,
            end_date_adjustment
        }
    }

    pub fn start_date_condition(&self) -> ThirtyAdjstmentCondition { self.start_date_condition }
    pub fn start_date_adjustment(&self) -> ThirtyAdjustment { self.start_date_adjustment }
    pub fn end_date_condition(&self) -> ThirtyAdjstmentCondition { self.end_date_condition }
    pub fn additional_start_date_condition(&self) -> ThirtyAdjstmentCondition { self.additional_start_date_condition }
    pub fn end_date_adjustment(&self) -> ThirtyAdjustment { self.end_date_adjustment }
}

impl DayCounterNumeratorGenerator for ThirtyNumeratorGenerator {
    fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<Arc<dyn DayCounterNumerator>, DayCounterGenerationError> { // 變更：Rc → Arc
        if let Some(schedule) = schedule_opt {
            let numerator = ThirtyNumerator::new(
                self.start_date_condition,
                self.start_date_adjustment,
                self.end_date_condition,
                self.additional_start_date_condition,
                self.end_date_adjustment,
                schedule,
            );
            Ok(Arc::new(numerator))
        } else {
            Err(DayCounterGenerationError::ScheduleNotGiven)
        }
    }
}
