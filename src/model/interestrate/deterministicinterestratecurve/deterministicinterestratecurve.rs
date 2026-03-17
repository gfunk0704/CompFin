use std::sync::Arc;

use chrono::NaiveDate;

use crate::model::interestrate::deterministicinterestratecurve::curvegenerationerror::CurveGenerationError;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::time::daycounter::daycounter::DayCounter;


pub trait InstantaneousForwardRateCurve {
    fn inst_forward(&self, date: NaiveDate) -> f64;
}


pub trait DeterministicInterestRateCurve: InstantaneousForwardRateCurve + InterestRateCurve {
    
}


pub trait DeterministicInterestRateCurveGenerator {
    fn reference_date(&self) -> NaiveDate;

    fn day_counter(&self) -> &Arc<DayCounter>;

    /// values的語意依實作而異：
    /// - PiecewisePolyInterestRateCurveGenerator：各節點的curve值，長度須與dates一致
    /// - Nelson-Siegel等參數化方法：模型參數（如β₀, β₁, β₂, τ）
    fn generate(
        &self,
        values: Vec<f64>,
    ) -> Result<Arc<dyn DeterministicInterestRateCurve>, CurveGenerationError>;
}