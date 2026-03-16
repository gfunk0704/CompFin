use std::sync::Arc;

use chrono::NaiveDate;

use crate:: model::interestrate::interestratecurve::InterestRateCurve;


pub trait InstantaneousForwardRateCurve {
    fn inst_forward(&self, date: NaiveDate) -> f64;
}


pub trait DeterministicInterestRateCurve: InstantaneousForwardRateCurve + InterestRateCurve {
    
}


pub trait DeterministicInterestRateCurveGenerator {
    fn generate(&self, values: Vec<f64>) -> Arc<dyn DeterministicInterestRateCurve>;
}