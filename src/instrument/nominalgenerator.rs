use std::cell::{
    Cell,
    RefCell,
    RefMut
};

use crate::interestrate::compounding::Compounding;
use crate::time::daycounter::daycounter::DayCounter;
use crate::time::schedule::schedule::Schedule;


pub struct NominalSetter {
    initial_nominal: Cell<f64>,
    rate: Cell<f64>
}

impl NominalSetter {
    pub fn new() -> Self {
        NominalSetter { 
            initial_nominal: Cell::new(1000000.0),
            rate: Cell::new(0.00)
        }
    }

    pub fn initial_nominal(&self) -> f64 {
        self.initial_nominal.get()
    }

    pub fn set_initial_nominal(&self, initial_nominal: f64) {
        self.initial_nominal.set(initial_nominal);
    }

    pub fn rate(&self) -> f64 {
        self.rate.get()
    }

    pub fn set_rate(&self, rate: f64) {
        self.rate.set(rate);
    }
}


pub trait NominalGenerator {
    fn setter(&self) -> RefMut<'_, NominalSetter>;

    // 回傳每個 schedule period 對應的名目本金，與 schedule_periods() 一對一對應
    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64>;
}


pub struct FixedNominalGenerator {
    setter: RefCell<NominalSetter>
}

impl FixedNominalGenerator {
    pub fn new() -> Self {
        FixedNominalGenerator { 
            setter: RefCell::new(NominalSetter::new())
        }
    }
}

impl NominalGenerator for FixedNominalGenerator {
    fn setter(&self) -> RefMut<'_, NominalSetter> {
        self.setter.borrow_mut()
    }

    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64> {
        let initial_nominal = self.setter.borrow().initial_nominal();
        // 每個 period 的名目本金都相同
        vec![initial_nominal; schedule.schedule_periods().len()]
    }
}


pub struct AccretingNominalGenerator {
    day_counter: DayCounter,
    compounding: Compounding,
    setter: RefCell<NominalSetter>
}

impl AccretingNominalGenerator {
    pub fn new(day_counter: DayCounter,
               compounding: Compounding) -> Self {
        AccretingNominalGenerator { 
            day_counter,
            compounding,
            setter: RefCell::new(NominalSetter::new())
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
    fn setter(&self) -> RefMut<'_, NominalSetter> {
        self.setter.borrow_mut()
    }

    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64> {
        let periods = schedule.schedule_periods();
        let rate = self.setter.borrow().rate();
        let mut current_nominal = self.setter.borrow().initial_nominal();

        let mut nominals = Vec::with_capacity(periods.len());
        nominals.push(current_nominal);

        // 最後一個 period 的名目本金由前一個 period 推算，不需要再複利
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