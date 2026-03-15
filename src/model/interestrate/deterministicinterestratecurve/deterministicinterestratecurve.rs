use chrono::NaiveDate;

use crate::model::interestrate::interestratecurve::InterestRateCurve;


pub trait InstantaneousForwardRateCurve {
    fn inst_forward(&self, date: NaiveDate) -> f64;
}


pub trait DeterministicInterestRateCurve: InstantaneousForwardRateCurve + InterestRateCurve {
    
}