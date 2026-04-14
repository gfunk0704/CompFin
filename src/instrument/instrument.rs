use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::time::period::Period;
use crate::value::cashflows::CashFlows;

use crate::market::market::Market;
use crate::pricingcondition::PricingCondition;

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum Position {
    Buy = 1,
    Sell = -1
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CurveFunction {
    ReceiveForward,
    PayForward,
    ProfitAndLossDiscount,
}




pub trait Instrument: Send + Sync {
    fn max_date(&self) -> NaiveDate;
    
    fn position(&self) -> Position;

    fn profit_and_loss_market(&self) -> &Arc<dyn Market>;

    fn curve_name_map(&self) -> &HashMap<CurveFunction, String>;

    fn is_linear(&self) -> bool;
}


pub trait InstrumentWithLinearFlows {
    fn past_pay_flows(&self, pricing_condition: &PricingCondition) -> CashFlows;

    fn past_receive_flows(&self, pricing_condition: &PricingCondition) -> CashFlows;

    fn projected_pay_flows(&self, forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>, pricing_condition: &PricingCondition) -> CashFlows;

    fn projected_receive_flows(&self, forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>, pricing_condition: &PricingCondition) -> CashFlows;

    /// 回傳 payment_date > cutoff 的 projected pay flows。
    ///
    /// 預設實作：先算全部 projected pay flows，再 retain_after(cutoff)。
    /// `Deposit` 與 `InterestRateSwap` 覆寫此方法，
    /// 利用 `partition_point` 在 `FlowObserver` 層直接截斷，
    /// 避免計算 cutoff 之前的 CompoundingRateIndex 等昂貴運算。
    fn projected_pay_flows_after(
        &self,
        cutoff: NaiveDate,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> CashFlows {
        let mut flows = self.projected_pay_flows(forward_curve_opt, pricing_condition);
        flows.retain_after(cutoff);
        flows
    }

    /// 回傳 payment_date <= cutoff 的 projected pay flows。
    ///
    /// 預設實作：先算全部 projected pay flows，再 retain_before_equal(cutoff)。
    fn projected_pay_flows_before_equal(
        &self,
        cutoff: NaiveDate,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> CashFlows {
        let mut flows = self.projected_pay_flows(forward_curve_opt, pricing_condition);
        flows.retain_before_equal(cutoff);
        flows
    }

    /// 回傳 payment_date > cutoff 的 projected receive flows。
    ///
    /// 預設實作：先算全部 projected receive flows，再 retain_after(cutoff)。
    /// `Deposit` 與 `InterestRateSwap` 覆寫此方法以達最佳效能。
    fn projected_receive_flows_after(
        &self,
        cutoff: NaiveDate,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> CashFlows {
        let mut flows = self.projected_receive_flows(forward_curve_opt, pricing_condition);
        flows.retain_after(cutoff);
        flows
    }

    /// 回傳 payment_date <= cutoff 的 projected receive flows。
    ///
    /// 預設實作：先算全部 projected receive flows，再 retain_before_equal(cutoff)。
    fn projected_receive_flows_before_equal(
        &self,
        cutoff: NaiveDate,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> CashFlows {
        let mut flows = self.projected_receive_flows(forward_curve_opt, pricing_condition);
        flows.retain_before_equal(cutoff);
        flows
    }
}


pub trait SimpleInstrument: Instrument + InstrumentWithLinearFlows {
}


pub trait SimpleInstrumentGenerator {
    fn generate_with_maturity_tenor(&self, 
                                    position: Position,
                                    trade_date: NaiveDate,
                                    maturity_tenor: Period,
                                    effective_date_opt: Option<NaiveDate>) -> Arc<dyn SimpleInstrument>;

    fn generate_with_maturity_date(&self,
                                   position: Position,   
                                   trade_date: NaiveDate,
                                   maturity_date: NaiveDate,
                                   effective_date_opt: Option<NaiveDate>) -> Arc<dyn SimpleInstrument>;

    /// 將市場原始報價轉換為 Bootstrapping 用的等效利率。
    ///
    /// 預設行為是直接回傳原值（適用於 Deposit、IRS 等）。
    /// 特殊的 Generator（如 SOFR Future）可以覆寫：
    /// ```ignore
    /// fn market_rate(&self, market_quote: f64) -> f64 {
    ///     0.01 * (100.0 - market_quote)
    /// }
    /// ```
    fn market_rate(&self, market_quote: f64) -> f64 {
        market_quote
    }
}
