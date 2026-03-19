use std::sync::RwLock;

use crate::interestrate::compounding::Compounding;
use crate::time::daycounter::daycounter::DayCounter;
use crate::time::schedule::schedule::Schedule;


pub struct NominalSetter {
    initial_nominal: RwLock<f64>,
    rate: RwLock<f64>,
}

impl NominalSetter {
    pub fn new() -> Self {
        Self {
            initial_nominal: RwLock::new(1_000_000.0),
            rate: RwLock::new(0.0),
        }
    }

    pub fn initial_nominal(&self) -> f64 {
        *self.initial_nominal.read().unwrap()
    }

    pub fn set_initial_nominal(&self, v: f64) {
        *self.initial_nominal.write().unwrap() = v;
    }

    pub fn rate(&self) -> f64 {
        *self.rate.read().unwrap()
    }

    pub fn set_rate(&self, v: f64) {
        *self.rate.write().unwrap() = v;
    }
}


pub trait NominalGenerator: Send + Sync {
    fn setter(&self) -> &NominalSetter;

    // 回傳每個schedule period對應的名目本金，與schedule_periods()一對一對應
    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64>;
}


pub struct FixedNominalGenerator {
    setter: NominalSetter,
}

impl FixedNominalGenerator {
    pub fn new() -> Self {
        Self {
            setter: NominalSetter::new(),
        }
    }
}

impl NominalGenerator for FixedNominalGenerator {
    fn setter(&self) -> &NominalSetter {
        &self.setter
    }

    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64> {
        let initial_nominal = self.setter.initial_nominal();
        vec![initial_nominal; schedule.schedule_periods().len()]
    }
}


pub struct AccretingNominalGenerator {
    day_counter: DayCounter,
    compounding: Compounding,
    setter: NominalSetter,
}

impl AccretingNominalGenerator {
    pub fn new(day_counter: DayCounter, compounding: Compounding) -> Self {
        Self {
            day_counter,
            compounding,
            setter: NominalSetter::new(),
        }
    }

    pub fn day_counter(&self) -> &DayCounter {
        &self.day_counter
    }

    pub fn compounding(&self) -> &Compounding {
        &self.compounding
    }
}

impl NominalGenerator for AccretingNominalGenerator {
    fn setter(&self) -> &NominalSetter {
        &self.setter
    }

    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64> {
        let periods = schedule.schedule_periods();
        let rate = self.setter.rate();
        let mut current_nominal = self.setter.initial_nominal();

        let mut nominals = Vec::with_capacity(periods.len());
        nominals.push(current_nominal);

        // 最後一個period的名目本金由前一個period推算，不需要再複利
        for period in periods.split_last().unwrap().1 {
            let cp = period.calculation_period();
            let tau = self.day_counter.year_fraction(
                cp.start_date(),
                cp.end_date(),
            );
            current_nominal *= self.compounding.future_value(rate, tau);
            nominals.push(current_nominal);
        }

        nominals
    }
}