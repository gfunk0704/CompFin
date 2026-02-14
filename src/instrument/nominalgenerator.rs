use std::cell::{
    Cell,
    RefCell,
    RefMut
};

use chrono::NaiveDate;

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

    pub fn set_initial_nominal(&self, initial_nominal: f64) -> () {
        self.initial_nominal.set(initial_nominal);
    }

    pub fn rate(&self) -> f64 {
        self.rate.get()
    }

    pub fn set_rate(&self, rate: f64) -> () {
        self.rate.set(rate);
    }
}



pub trait NominalGenerator {
    fn setter(&self) -> RefMut<'_, NominalSetter>;

    fn generate_nominal(&self, 
                        schedule: &Schedule) -> Vec<(NaiveDate, f64)>;
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

    fn generate_nominal(&self, 
                        schedule: &Schedule) -> Vec<(NaiveDate, f64)> {
        let initial_nominal = self.setter.borrow().initial_nominal();
        let mut nominals: Vec<(NaiveDate, f64)> = Vec::new();
        for period in schedule.schedule_periods() {
            nominals.push((period.calculation_period().start_date(), initial_nominal));
        }   
        nominals        
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

    fn generate_nominal(&self, 
                        schedule: &Schedule) -> Vec<(NaiveDate, f64)> {

        let mut nominals: Vec<(NaiveDate, f64)> = Vec::new();
        let mut current_nominal = self.setter.borrow().initial_nominal();
        let rate = self.setter.borrow().rate();
        nominals.push((schedule.schedule_periods()[0].calculation_period().start_date(), current_nominal));

        for period in schedule.schedule_periods().split_last().unwrap().1.iter() {
            let start_date = period.calculation_period().start_date();
            let end_date = period.calculation_period().end_date();
            let tau = self.day_counter.year_fraction(start_date, end_date);
            current_nominal *= self.compounding.future_value(rate, tau);
            nominals.push((start_date, current_nominal));

        }   

        nominals        
    }        
}