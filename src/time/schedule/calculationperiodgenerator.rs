use std::rc::Rc;

use chrono::{
    Datelike, 
    NaiveDate
};
use serde::Deserialize;

use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::period::Period;
use crate::time::schedule::scheduleperiod::CalculationPeriod;
use crate::time::schedule::generationdirection::GenerationDirection;
use crate::time::schedule::stubadjuster::{
    StubAdjuster, 
    StubConvention
};

#[derive(PartialEq, Eq, Clone, Copy, Deserialize)]
pub enum GenerationMode {
    Normal,
    Recursive,
}

struct EndCriteria {
    last_date: NaiveDate,
    comparison_operator: fn(NaiveDate, NaiveDate) -> bool,
}

impl EndCriteria {
    pub fn new(
        last_date: NaiveDate,
        comparison_operator: fn(NaiveDate, NaiveDate) -> bool,
    ) -> EndCriteria {
        EndCriteria {
            last_date,
            comparison_operator,
        }
    }

    pub fn satisfy(&self, d: NaiveDate) -> bool {
        (self.comparison_operator)(d, self.last_date)
    }

    pub fn last_date(&self) -> NaiveDate {
        self.last_date
    }
}

#[derive(Clone, Copy)]
struct EndCriteriaGenerator {
    forward: bool,
    comparison_operator: fn(NaiveDate, NaiveDate) -> bool,
}

impl EndCriteriaGenerator {
    pub fn new(forward: bool) -> EndCriteriaGenerator {
        let comparison_operator = if forward {
            |d1: NaiveDate, d2: NaiveDate| d1 >= d2
        } else {
            |d1: NaiveDate, d2: NaiveDate| d1 <= d2
        };
        EndCriteriaGenerator {
            forward,
            comparison_operator,
        }
    }

    pub fn generate(&self, start_date: NaiveDate, end_date: NaiveDate) -> EndCriteria {
        let last_date = if self.forward { end_date } else { start_date };
        EndCriteria::new(last_date, self.comparison_operator)
    }
}

#[derive(Clone)]
pub struct CalculationPeriodGenerator {
    start_lag: i32,
    frequency: Period,
    freq_adjuster: BusinessDayAdjuster,
    mat_adjuster: BusinessDayAdjuster,
    mode: GenerationMode,
    direction: GenerationDirection,
    stub_adjuster: StubAdjuster,
    end_criteria_generator: EndCriteriaGenerator,
}

impl CalculationPeriodGenerator {
    pub fn new(
        start_lag: i32,
        frequency: Period,
        freq_adjuster: BusinessDayAdjuster,
        mat_adjuster: BusinessDayAdjuster,
        mode: GenerationMode,
        direction: GenerationDirection,
        stub_convention: StubConvention,
    ) -> CalculationPeriodGenerator {
        let forward = direction == GenerationDirection::Forward;

        CalculationPeriodGenerator {
            start_lag,
            frequency,
            freq_adjuster,
            mat_adjuster,
            mode,
            direction,
            stub_adjuster: StubAdjuster::new(stub_convention, forward),
            end_criteria_generator: EndCriteriaGenerator::new(forward),
        }
    }

    pub fn start_lag(&self) -> i32 {
        self.start_lag
    }

    pub fn frequency(&self) -> Period {
        self.frequency
    }

    pub fn freq_adjuster(&self) -> &BusinessDayAdjuster {
        &self.freq_adjuster
    }

    pub fn mat_adjuster(&self) -> &BusinessDayAdjuster {
        &self.mat_adjuster
    }

    pub fn direction(&self) -> GenerationDirection {
        self.direction
    }

    pub fn stub_convention(&self) -> StubConvention {
        self.stub_adjuster.convention()
    }

    pub fn generate_extension_periods(
        &self,
        calendar: &Rc<dyn HolidayCalendar>,
        horizon: NaiveDate,
        maturity: NaiveDate,
    ) -> Option<Vec<CalculationPeriod>> {
        self.generate_from_maturity_date_impl(calendar, horizon, maturity, false)
    }

    pub fn generate_from_maturity_date(
        &self,
        calendar: &Rc<dyn HolidayCalendar>,
        horizon: NaiveDate,
        maturity: NaiveDate,
    ) -> Option<Vec<CalculationPeriod>> {
        self.generate_from_maturity_date_impl(calendar, horizon, maturity, true)
    }

    pub fn generate_from_maturity_tenor(
        &self,
        calendar: &Rc<dyn HolidayCalendar>,
        horizon: NaiveDate,
        maturity: Period,
    ) -> Option<Vec<CalculationPeriod>> {
        let start_date = calendar.shift_n_business_day(horizon, self.start_lag);
        let maturity_date = self
            .mat_adjuster
            .from_tenor_to_date(start_date, maturity, calendar);
        self.generate_from_maturity_date(calendar, horizon, maturity_date)
    }

    fn generate_from_maturity_date_impl(
        &self,
        calendar: &Rc<dyn HolidayCalendar>,
        horizon: NaiveDate,
        maturity: NaiveDate,
        apply_stub_adjuster: bool,
    ) -> Option<Vec<CalculationPeriod>> {
        let start_date = calendar.shift_n_business_day(horizon, self.start_lag);
        let end_criteria = self.end_criteria_generator.generate(start_date, maturity);
        let forward = self.direction() == GenerationDirection::Forward;
        let begin_date = if forward { start_date } else { maturity };

        if end_criteria.satisfy(begin_date) {
            return None;
        }

        let create_calculation_period = if forward {
            |d1: NaiveDate, d2: NaiveDate| CalculationPeriod::new(d1, d2)
        } else {
            |d1: NaiveDate, d2: NaiveDate| CalculationPeriod::new(d2, d1)
        };

        let mut calculation_periods: Vec<CalculationPeriod> = Vec::new();
        let mut d1 = begin_date;
        let mut d2;

        match self.mode {
            GenerationMode::Normal => {
                let mut step = 0;
                let step_size = self.frequency.number() * (self.direction as i32);
                let unit = self.frequency.unit();

                if self.freq_adjuster.eom() {
                    let mut is_eom = false;

                    loop {
                        if calendar.last_business_day_of_month(d1.year(), d1.month()) == d1 {
                            is_eom = true;
                            break;
                        }
                        step += step_size;
                        d2 = self.freq_adjuster.from_tenor_to_date(
                            begin_date,
                            Period::new(step, unit),
                            calendar,
                        );
                        calculation_periods.push(create_calculation_period(d1, d2));
                        d1 = d2;
                        if end_criteria.satisfy(d1) {
                            break;
                        }
                    }

                    if is_eom {
                        while !end_criteria.satisfy(d1) {
                            step += step_size;
                            let unadjusted = d1 + Period::new(step, unit);
                            d2 = calendar.last_business_day_of_month(
                                unadjusted.year(),
                                unadjusted.month(),
                            );
                            calculation_periods.push(create_calculation_period(d1, d2));
                            d1 = d2;
                        }
                    }
                } else {
                    loop {
                        step += step_size;
                        d2 = self.freq_adjuster.from_tenor_to_date(
                            begin_date,
                            Period::new(step, unit),
                            calendar,
                        );
                        calculation_periods.push(create_calculation_period(d1, d2));
                        d1 = d2;
                        if end_criteria.satisfy(d1) {
                            break;
                        }
                    }
                }
            }
            GenerationMode::Recursive => {
                loop {
                    d2 = self
                        .freq_adjuster
                        .from_tenor_to_date(d1, self.frequency, calendar);
                    calculation_periods.push(create_calculation_period(d1, d2));
                    d1 = d2;
                    if end_criteria.satisfy(d1) {
                        break;
                    }
                }
            }
        }

        if !forward {
            calculation_periods.reverse();
        }

        if apply_stub_adjuster {
            Some(
                self.stub_adjuster
                    .adjust(end_criteria.last_date(), calculation_periods),
            )
        } else {
            Some(calculation_periods)
        }
    }
}