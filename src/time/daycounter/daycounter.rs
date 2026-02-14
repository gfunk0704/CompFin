use std::rc::Rc;

use chrono::{Days, NaiveDate};

use super::super::schedule::schedule::Schedule;

pub trait DayCounterNumerator {
    fn days_between(&self, d1: NaiveDate, d2: NaiveDate) -> f64;
}

#[derive(Debug)]
pub enum DayCounterGenerationError {
    ScheduleNotGiven,
    IrregularFrequencyForICMADominator,
}

impl DayCounterGenerationError {
    pub fn to_string(&self) -> String {
        match self {
            DayCounterGenerationError::ScheduleNotGiven => {
                "Schedule not given for day counter generation".to_string()
            }
            DayCounterGenerationError::IrregularFrequencyForICMADominator => {
                "Irregular frequency given for ICMA actual day count dominator generation"
                    .to_string()
            }
        }
    }
}

pub trait DayCounterNumeratorGenerator {
    fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<Rc<dyn DayCounterNumerator>, DayCounterGenerationError>;
}

pub trait DayCounterDominator {
    fn year_fraction(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        numerator: &Rc<dyn DayCounterNumerator>,
    ) -> f64;
}

pub trait DayCounterDominatorGenerator {
    fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<Rc<dyn DayCounterDominator>, DayCounterGenerationError>;
}

pub struct DayCounter {
    numerator: Rc<dyn DayCounterNumerator>,
    dominator: Rc<dyn DayCounterDominator>,
    shift_days1: Days,
    shift_days2: Days,
}

impl DayCounter {
    pub fn new(
        include_d1: bool,
        include_d2: bool,
        numerator: Rc<dyn DayCounterNumerator>,
        dominator: Rc<dyn DayCounterDominator>,
    ) -> DayCounter {
        DayCounter {
            numerator,
            dominator,
            shift_days1: if include_d1 {
                Days::new(1)
            } else {
                Days::new(0)
            },
            shift_days2: if include_d2 {
                Days::new(0)
            } else {
                Days::new(1)
            },
        }
    }

    pub fn include_d1(&self) -> bool {
        self.shift_days1 == Days::new(1)
    }

    pub fn include_d2(&self) -> bool {
        self.shift_days2 == Days::new(0)
    }

    /// Calculate the year fraction between two dates.
    ///
    /// # Bug Fix
    /// Fixed condition: was `d2 > d1`, now correctly `d1 > d2` for reversal case.
    ///
    /// # Optimization
    /// Avoid recursion by directly computing the negated value.
    pub fn year_fraction(&self, d1: NaiveDate, d2: NaiveDate) -> f64 {
        if d1 == d2 {
            0.0
        } else if d1 > d2 {
            // Fixed: was `d2 > d1`, which was backwards
            // Optimization: avoid recursion, compute directly
            let start_date = d2 + self.shift_days1;
            let end_date = d1 + self.shift_days2;
            -self
                .dominator
                .year_fraction(start_date, end_date, &self.numerator)
        } else {
            let start_date = d1 + self.shift_days1;
            let end_date = d2 + self.shift_days2;
            self.dominator
                .year_fraction(start_date, end_date, &self.numerator)
        }
    }
}

pub struct DayCounterGenerator {
    numerator_generator: Rc<dyn DayCounterNumeratorGenerator>,
    dominator_generator: Rc<dyn DayCounterDominatorGenerator>,
    include_d1: bool,
    include_d2: bool,
}

impl DayCounterGenerator {
    pub fn new(
        numerator_generator: Rc<dyn DayCounterNumeratorGenerator>,
        dominator_generator: Rc<dyn DayCounterDominatorGenerator>,
        include_d1: bool,
        include_d2: bool,
    ) -> DayCounterGenerator {
        DayCounterGenerator {
            numerator_generator,
            dominator_generator,
            include_d1,
            include_d2,
        }
    }

    pub fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<DayCounter, DayCounterGenerationError> {
        let numerator = self.numerator_generator.generate(schedule_opt)?;
        let dominator = self.dominator_generator.generate(schedule_opt)?;
        Ok(DayCounter::new(
            self.include_d1,
            self.include_d2,
            numerator,
            dominator,
        ))
    }
}
