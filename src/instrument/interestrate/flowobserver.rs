use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::leg::legcharacters::LegCharacters;
use crate::math::round::round;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;

pub struct FlowObserver {
    ref_leg_characters: Arc<dyn LegCharacters>,
    nominal: f64,
    i: usize
}   

impl FlowObserver {
    pub fn new(ref_leg_characters: Arc<dyn LegCharacters>, 
               nominal: f64, 
               i: usize) -> Self {
        Self {
            ref_leg_characters,
            nominal,
            i
        }
    }

    pub fn nominal(&self) -> f64 {
        self.nominal
    }

    pub fn payment_date(&self) -> NaiveDate {
        self.ref_leg_characters.
            generic_characters().
            schedule().
            schedule_periods()[self.i].
            payment_date()
    }

    pub fn projected_flow(&self,
                          forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
                          pricing_condition: &PricingCondition,
                          rounding_digits_opt: Option<u32>) -> f64 {
        let flow = self.ref_leg_characters.evaluate_flow(self.i, forward_curve_opt, pricing_condition) * self.nominal;
        if rounding_digits_opt.is_some() {
            round(flow, rounding_digits_opt.unwrap())
        } else {
            flow
        }
    }   

    pub fn ref_leg_characters(&self) -> &Arc<dyn LegCharacters> {
        &self.ref_leg_characters
    }

    pub fn i(&self) -> usize {
        self.i
    }
}



pub struct CapitalizationFlow {
    amount: f64,
    payment_date: NaiveDate
}

impl CapitalizationFlow {
    pub fn new(amount: f64, payment_date: NaiveDate) -> Self {
        Self {
            amount,
            payment_date
        }
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn payment_date(&self) -> NaiveDate {
        self.payment_date
    }
}

