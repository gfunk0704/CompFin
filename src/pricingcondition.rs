use chrono::NaiveDate;


pub struct DacimalRounding {
    deterministic_flow: bool,
    estimated_flow: bool
}


impl DacimalRounding {
    pub fn new(deterministic_flow: bool,
               estimated_flow: bool) -> DacimalRounding {
        DacimalRounding {
            deterministic_flow: deterministic_flow,
            estimated_flow: estimated_flow
        }
    }

    pub fn deterministic_flow(&self) -> bool {
        self.deterministic_flow
    }

    pub fn estimated_flow(&self) -> bool {
        self.estimated_flow
    }

    pub fn apply_rounding(&self) -> bool {
        self.deterministic_flow || self.estimated_flow
    }
}

pub struct PricingCondition {
    horizon: NaiveDate,
    include_horizon_flow: bool,
    estimate_horizon_index: bool,
    dacimal_rounding: DacimalRounding
}


impl PricingCondition {
    pub fn new(horizon: NaiveDate,
               include_horizon_flow: bool,
               estimate_horizon_index: bool,
               dacimal_rounding: DacimalRounding) -> PricingCondition {
        PricingCondition {
            horizon: horizon,
            include_horizon_flow: include_horizon_flow,
            estimate_horizon_index: estimate_horizon_index,
            dacimal_rounding: dacimal_rounding
        }
    }

    pub fn horizon(&self) -> &NaiveDate {
        &self.horizon
    }

    pub fn include_horizon_flow(&self) -> &bool {
        &self.include_horizon_flow
    }

    pub fn estimate_horizon_index(&self) -> &bool {
        &self.estimate_horizon_index
    }

    pub fn dacimal_rounding(&self) -> &DacimalRounding {
        &self.dacimal_rounding
    }
}