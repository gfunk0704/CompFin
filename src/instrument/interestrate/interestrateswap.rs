use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::instrument::{
    CurveFunction,
    Instrument,
    InstrumentWithLinearFlows,
    Position,
};
use crate::instrument::interestrate::flowobserver::FlowObserver;
use crate::instrument::nominalgenerator::NominalGenerator;
use crate::instrument::leg::legcharacters::LegCharacters;
use crate::market::market::Market;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::value::cashflows::CashFlows;


pub struct InterestRateSwap {
    position: Position,
    profit_and_loss_market: Arc<dyn Market>,
    pay_leg_characters: Arc<dyn LegCharacters>,
    pay_leg_nominal_generator: Arc<dyn NominalGenerator>,
    pay_leg_flow_observer_list: Vec<FlowObserver>,
    receive_leg_characters: Arc<dyn LegCharacters>,
    receive_leg_flow_observer_list: Vec<FlowObserver>,
    receive_leg_nominal_generator: Arc<dyn NominalGenerator>,
    curve_name_map: HashMap<CurveFunction, String>,
}

impl InterestRateSwap {
    pub fn new(
        position: Position,
        profit_and_loss_market: Arc<dyn Market>,
        pay_leg_characters: Arc<dyn LegCharacters>,
        pay_leg_nominal_generator: Arc<dyn NominalGenerator>,
        receive_leg_characters: Arc<dyn LegCharacters>,
        receive_leg_nominal_generator: Arc<dyn NominalGenerator>,
    ) -> Self {
        let pay_leg_flow_observer_list = Self::build_flow_observer_list(
            &pay_leg_characters,
            &pay_leg_nominal_generator,
        );
        let receive_leg_flow_observer_list = Self::build_flow_observer_list(
            &receive_leg_characters,
            &receive_leg_nominal_generator,
        );

        // IRS兩條腿共用同一個market的discount curve
        // pay/receive各自的forward curve由各自的leg_characters提供（floating才有）
        let mut curve_name_map: HashMap<CurveFunction, String> = HashMap::new();
        curve_name_map.insert(
            CurveFunction::PayDiscount,
            profit_and_loss_market.discount_curve_name().to_string(),
        );
        curve_name_map.insert(
            CurveFunction::ReceiveDiscount,
            profit_and_loss_market.discount_curve_name().to_string(),
        );
        if let Some(name) = pay_leg_characters.reference_curve_name() {
            curve_name_map.insert(CurveFunction::PayForward, name.to_string());
        }
        if let Some(name) = receive_leg_characters.reference_curve_name() {
            curve_name_map.insert(CurveFunction::ReceiveForward, name.to_string());
        }

        Self {
            position,
            profit_and_loss_market,
            pay_leg_characters,
            pay_leg_nominal_generator,
            pay_leg_flow_observer_list,
            receive_leg_characters,
            receive_leg_nominal_generator,
            receive_leg_flow_observer_list,
            curve_name_map,
        }
    }

    pub fn pay_leg_characters(&self) -> &Arc<dyn LegCharacters> {
        &self.pay_leg_characters
    }

    pub fn receive_leg_characters(&self) -> &Arc<dyn LegCharacters> {
        &self.receive_leg_characters
    }

    pub fn pay_leg_nominal_generator(&self) -> &Arc<dyn NominalGenerator> {
        &self.pay_leg_nominal_generator
    }

    pub fn receive_leg_nominal_generator(&self) -> &Arc<dyn NominalGenerator> {
        &self.receive_leg_nominal_generator
    }

    fn build_flow_observer_list(
        leg_characters: &Arc<dyn LegCharacters>,
        nominal_generator: &Arc<dyn NominalGenerator>,
    ) -> Vec<FlowObserver> {
        let schedule = leg_characters.generic_characters().schedule();
        nominal_generator
            .generate_nominal(schedule)
            .into_iter()
            .enumerate()
            .map(|(i, nominal)| FlowObserver::new(leg_characters.clone(), nominal, i))
            .collect()
    }

    // past flows的共用邏輯：找到所有payment_date已過horizon的flows
    fn collect_past_flows(
        flow_observer_list: &Vec<FlowObserver>,
        pricing_condition: &PricingCondition,
        rounding_digits_opt: Option<u32>,
        sign: f64,
    ) -> CashFlows {
        let mut cash_flows = CashFlows::new();
        // past flow的include_horizon邏輯與projected相反
        let include_horizon = !(*pricing_condition.include_horizon_flow());
        let horizon = *pricing_condition.horizon();

        if flow_observer_list.first().unwrap().payment_date() > horizon ||
           (flow_observer_list.first().unwrap().payment_date() == horizon && include_horizon) {
            return cash_flows;
        }

        let pred = |fo: &FlowObserver| {
            if include_horizon {
                fo.payment_date() <= horizon
            } else {
                fo.payment_date() < horizon
            }
        };

        let pos = flow_observer_list.partition_point(pred);
        for i in 0..pos {
            cash_flows[&flow_observer_list[i].payment_date()] +=
                sign * flow_observer_list[i].projected_flow(None, pricing_condition, rounding_digits_opt, None);
        }

        cash_flows
    }

    // projected flows的共用邏輯：找到所有payment_date在horizon之後的flows
    fn collect_projected_flows(
        flow_observer_list: &Vec<FlowObserver>,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
        flow_rounding_digits_opt: Option<u32>,
        index_rounding_digits_opt: Option<u32>,
        sign: f64,
    ) -> CashFlows {
        let mut cash_flows = CashFlows::new();
        let include_horizon = *pricing_condition.include_horizon_flow();
        let horizon = *pricing_condition.horizon();

        // 若最後一個flow都已是過去，直接回傳空的cash flows
        if flow_observer_list.last().unwrap().payment_date() < horizon ||
           (flow_observer_list.last().unwrap().payment_date() == horizon && !include_horizon) {
            return cash_flows;
        }

        let pred = |fo: &FlowObserver| {
            if include_horizon {
                fo.payment_date() <= horizon
            } else {
                fo.payment_date() < horizon
            }
        };

        let pos = flow_observer_list.partition_point(pred);
        for i in pos..flow_observer_list.len() {
            cash_flows[&flow_observer_list[i].payment_date()] +=
                sign * flow_observer_list[i].projected_flow(
                    forward_curve_opt,
                    pricing_condition,
                    flow_rounding_digits_opt,
                    index_rounding_digits_opt,
                );
        }

        cash_flows
    }
}


impl Instrument for InterestRateSwap {
    fn max_date(&self) -> NaiveDate {
        self.pay_leg_characters.max_date()
            .max(self.receive_leg_characters.max_date())
    }

    fn position(&self) -> Position {
        self.position
    }

    fn profit_and_loss_market(&self) -> &Arc<dyn Market> {
        &self.profit_and_loss_market
    }

    fn curve_name_map(&self) -> &HashMap<CurveFunction, String> {
        &self.curve_name_map
    }

    fn is_linear(&self) -> bool {
        true
    }
}


impl InstrumentWithLinearFlows for InterestRateSwap {
    fn past_pay_flows(&self, pricing_condition: PricingCondition) -> CashFlows {
        let digits = self.profit_and_loss_market.settlement_currency().digits();
        Self::collect_past_flows(
            &self.pay_leg_flow_observer_list,
            &pricing_condition,
            Some(digits),
            1.0,
        )
    }

    fn past_receive_flows(&self, pricing_condition: PricingCondition) -> CashFlows {
        let digits = self.profit_and_loss_market.settlement_currency().digits();
        Self::collect_past_flows(
            &self.receive_leg_flow_observer_list,
            &pricing_condition,
            Some(digits),
            1.0,
        )
    }

    fn projected_pay_flows(
        &self,
        forward_curve_opt: Option<Arc<dyn InterestRateCurve>>,
        pricing_condition: PricingCondition,
    ) -> CashFlows {
        let digits = self.profit_and_loss_market.settlement_currency().digits();
        let is_floating = forward_curve_opt.is_some();
        let flow_rounding = if is_floating {
            pricing_condition.floating_flow_rounding_digits(digits)
        } else {
            pricing_condition.fixed_flow_rounding_digits(digits)
        };
        let index_rounding = if is_floating {
            pricing_condition.floating_index_rounding_digits(digits)
        } else {
            None
        };

        Self::collect_projected_flows(
            &self.pay_leg_flow_observer_list,
            forward_curve_opt.as_ref(),
            &pricing_condition,
            flow_rounding,
            index_rounding,
            1.0,
        )
    }

    fn projected_receive_flows(
        &self,
        forward_curve_opt: Option<Arc<dyn InterestRateCurve>>,
        pricing_condition: PricingCondition,
    ) -> CashFlows {
        let digits = self.profit_and_loss_market.settlement_currency().digits();
        let is_floating = forward_curve_opt.is_some();
        let flow_rounding = if is_floating {
            pricing_condition.floating_flow_rounding_digits(digits)
        } else {
            pricing_condition.fixed_flow_rounding_digits(digits)
        };
        let index_rounding = if is_floating {
            pricing_condition.floating_index_rounding_digits(digits)
        } else {
            None
        };

        Self::collect_projected_flows(
            &self.receive_leg_flow_observer_list,
            forward_curve_opt.as_ref(),
            &pricing_condition,
            flow_rounding,
            index_rounding,
            1.0,
        )
    }
}