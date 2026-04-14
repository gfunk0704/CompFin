use chrono::NaiveDate;


pub struct DecimalRounding {
    deterministic_flow: bool,
    // floating rate的rounding分兩個層級：
    // estimated_index: index測量結果（rate）四捨五入，在evaluate_flow內部發生
    // estimated_flow:  最終flow金額四捨五入，在projected_flow最外層發生
    // 兩者可獨立開關，對應Murex的三種模式
    estimated_index: bool,
    estimated_flow: bool,
}


impl DecimalRounding {
    pub fn new(deterministic_flow: bool,
               estimated_index: bool,
               estimated_flow: bool) -> DecimalRounding {
        DecimalRounding {
            deterministic_flow,
            estimated_index,
            estimated_flow,
        }
    }

    pub fn deterministic_flow(&self) -> bool {
        self.deterministic_flow
    }

    pub fn estimated_index(&self) -> bool {
        self.estimated_index
    }

    pub fn estimated_flow(&self) -> bool {
        self.estimated_flow
    }
}

pub struct PricingCondition {
    horizon: NaiveDate,
    include_horizon_flow: bool,
    estimate_horizon_index: bool,
    decimal_rounding: DecimalRounding
}


impl PricingCondition {
    pub fn new(horizon: NaiveDate,
               include_horizon_flow: bool,
               estimate_horizon_index: bool,
               decimal_rounding: DecimalRounding) -> PricingCondition {
        PricingCondition {
            horizon,
            include_horizon_flow,
            estimate_horizon_index,
            decimal_rounding,
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

    // ── rounding決策方法 ──────────────────────────────────────────────────────
    // 呼叫端只需傳入幣別digits，不需要知道DecimalRounding的內部旗標
    // 決策邏輯集中在PricingCondition，保持「知道脈絡的物件做決策」的原則

    /// fixed rate leg的flow金額四捨五入
    pub fn fixed_flow_rounding_digits(&self, currency_digits: u32) -> Option<u32> {
        self.decimal_rounding.deterministic_flow().then_some(currency_digits)
    }

    /// floating rate leg的index rate四捨五入（在evaluate_flow內部，乘leverage前）
    pub fn floating_index_rounding_digits(&self, currency_digits: u32) -> Option<u32> {
        self.decimal_rounding.estimated_index().then_some(currency_digits)
    }

    /// floating rate leg的flow金額四捨五入（在projected_flow最外層）
    pub fn floating_flow_rounding_digits(&self, currency_digits: u32) -> Option<u32> {
        self.decimal_rounding.estimated_flow().then_some(currency_digits)
    }
}