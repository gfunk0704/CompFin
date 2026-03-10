use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::interestrate::flowobserver::{
    CapitalizationFlow, 
    FlowObserver
};
use crate::instrument::instrument::{
    CurveFunction, 
    Instrument, 
    InstrumentWithLinearFlows, 
    Position
};
use crate::instrument::leg::legcharacters::LegCharacters;
use crate::market::market::Market;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::value::cashflows::CashFlows;


pub struct Deposit {
    position: Position,
    nominal: f64,
    leg_characters: Arc<dyn LegCharacters>,
    profit_and_loss_market: Arc<dyn Market>,
    capitalization_flow_list: Vec<CapitalizationFlow>,
    flow_oberver_list: Vec<FlowObserver>,
    curve_name_map: HashMap<CurveFunction, String>
}


impl Deposit {
    pub fn new(position: Position,
               nominal: f64,
               leg_characters: Arc<dyn LegCharacters>,
               profit_and_loss_market: Arc<dyn Market>) -> Self {
        let schedule = leg_characters.generic_characters().schedule();
        let mut capitalization_flow_list: Vec<CapitalizationFlow> = Vec::with_capacity(2);

        // 期初本金會在第一次的start date被存入deposit
        let initial_capitalization_flow = CapitalizationFlow::new(
            -nominal,
            schedule.
            schedule_periods().
            first().
            unwrap().
            calculation_period().
            start_date()
        );
        capitalization_flow_list.push(initial_capitalization_flow);
        // 最後一次payment date本金會被取出
        let maturity_capitalization_flow = CapitalizationFlow::new(
            nominal,
            schedule.
            schedule_periods().
            last().
            unwrap().
            payment_date()
        );
        capitalization_flow_list.push(maturity_capitalization_flow);

        // 中間配息次數對齊schedule_periods
        let mut flow_oberver_list: Vec<FlowObserver> = Vec::with_capacity(schedule.schedule_periods().len());
        for i in 0..schedule.schedule_periods().len() {
            flow_oberver_list.push(FlowObserver::new(leg_characters.clone(), nominal, i));
        }

        // curve_name_map的建構邏輯：Deposit只有一個leg，所以reference curve就是profit_and_loss_market的discount curve；如果leg有reference curve（floating leg才會有），則加入forward curve
        let mut curve_name_map: HashMap<CurveFunction, String> = HashMap::new();
        curve_name_map.insert(CurveFunction::ReceiveDiscount, profit_and_loss_market.discount_curve_name().to_string());
        curve_name_map.insert(CurveFunction::PayDiscount, profit_and_loss_market.discount_curve_name().to_string());

        if leg_characters.reference_curve_name().is_some() {
            curve_name_map.insert(CurveFunction::ReceiveForward, leg_characters.reference_curve_name().unwrap().to_string());
        }


        Self {
            position,
            nominal,
            leg_characters,
            profit_and_loss_market,
            capitalization_flow_list,
            flow_oberver_list,
            curve_name_map
        }
    }

    pub fn nominal(&self) -> f64 {
        self.nominal
    }

    pub fn leg_characters(&self) -> Arc<dyn LegCharacters> {
        self.leg_characters.clone()
    }

    pub fn capitalization_flow_list(&self) -> &Vec<CapitalizationFlow> {
        &self.capitalization_flow_list
    }

    pub fn flow_oberver_list(&self) -> &Vec<FlowObserver> {
        &self.flow_oberver_list
    }
}


impl Instrument for Deposit {
    fn max_date(&self) -> NaiveDate {
        self.leg_characters.max_date()
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


impl InstrumentWithLinearFlows for Deposit {
    fn past_receive_flows(&self, pricing_condition: PricingCondition) -> CashFlows {
        let mut cash_flows: CashFlows = CashFlows::new();
        // include_horizon_flow是對projection flow，對past flow會是相反邏輯
        let include_horizon: bool = !(*pricing_condition.include_horizon_flow()); 
        let horizon: NaiveDate = *pricing_condition.horizon();
        let receive_nominal_flow = self.capitalization_flow_list().last().unwrap();
        let rounding_digits_opt = Some(self.profit_and_loss_market.settlement_currency().digits());

        if  receive_nominal_flow.payment_date() < horizon || 
            (receive_nominal_flow.payment_date() == horizon && !include_horizon) {
            cash_flows[&receive_nominal_flow.payment_date()] += receive_nominal_flow.amount();
        }

        if self.flow_oberver_list().first().unwrap().payment_date() > horizon ||
           (self.flow_oberver_list().first().unwrap().payment_date() == horizon && include_horizon) {
            return cash_flows;
        }

        let pred = |flow_observer: &FlowObserver| {
            if include_horizon {   // 只是讀一個已捕獲的 bool，非常便宜
                flow_observer.payment_date() <= horizon
            } else {
                flow_observer.payment_date() < horizon
            }
        };

        let pos =self.flow_oberver_list.partition_point(pred);

        for i in 0..pos {
            cash_flows[&self.flow_oberver_list()[i].payment_date()] += self.
                flow_oberver_list()[i].
                projected_flow(None, &pricing_condition, rounding_digits_opt, None);
        }

        cash_flows
    }

    fn past_pay_flows(&self, pricing_condition: PricingCondition) -> CashFlows {
        
        let mut cash_flows: CashFlows = CashFlows::new();
        let pay_nominal_flow = self.capitalization_flow_list().first().unwrap();
        let include_horizon: bool = !(*pricing_condition.include_horizon_flow()); 
        let horizon: NaiveDate = *pricing_condition.horizon();

        if  pay_nominal_flow.payment_date() < horizon || 
            (pay_nominal_flow.payment_date() == horizon && !include_horizon) {
            cash_flows[&pay_nominal_flow.payment_date()] -= pay_nominal_flow.amount();
        }

        cash_flows
    }

    fn projected_pay_flows(&self, _forward_curve_opt: Option<Arc<dyn InterestRateCurve>>, pricing_condition: PricingCondition) -> CashFlows {
        let mut cash_flows: CashFlows = CashFlows::new();
        // projected flows的include_horizon邏輯與past flows相反（不inverted）
        let include_horizon: bool = *pricing_condition.include_horizon_flow();
        let pricing_date: NaiveDate = *pricing_condition.horizon();
        let pay_nominal_flow = self.capitalization_flow_list().first().unwrap();

        // 期初本金支出若還沒發生，則屬於projected pay flow
        if pay_nominal_flow.payment_date() > pricing_date ||
           (pay_nominal_flow.payment_date() == pricing_date && include_horizon) {
            cash_flows[&pay_nominal_flow.payment_date()] -= pay_nominal_flow.amount();
        }

        cash_flows
    }

    fn projected_receive_flows(&self, forward_curve_opt: Option<Arc<dyn InterestRateCurve>>, pricing_condition: PricingCondition) -> CashFlows {
        let mut cash_flows: CashFlows = CashFlows::new();
        // projected flows的include_horizon邏輯與past flows相反（不inverted）
        let include_horizon: bool = *pricing_condition.include_horizon_flow();
        let horizon: NaiveDate = *pricing_condition.horizon();
        let receive_nominal_flow = self.capitalization_flow_list().last().unwrap();

        // 期末本金回收若還沒發生，則屬於projected receive flow
        if receive_nominal_flow.payment_date() > horizon ||
           (receive_nominal_flow.payment_date() == horizon && include_horizon) {
            cash_flows[&receive_nominal_flow.payment_date()] += receive_nominal_flow.amount();
        }

        // 若連最後一個flow都已是過去，不需要繼續
        if self.flow_oberver_list().last().unwrap().payment_date() < horizon ||
           (self.flow_oberver_list().last().unwrap().payment_date() == horizon && !include_horizon) {
            return cash_flows;
        }

        // rounding決策委託給PricingCondition，呼叫端只需提供幣別digits
        let digits = self.profit_and_loss_market.settlement_currency().digits();
        let is_floating = forward_curve_opt.is_some();
        let flow_rounding_digits_opt: Option<u32> = if is_floating {
            pricing_condition.floating_flow_rounding_digits(digits)
        } else {
            pricing_condition.fixed_flow_rounding_digits(digits)
        };
        let index_rounding_digits_opt: Option<u32> = if is_floating {
            pricing_condition.floating_index_rounding_digits(digits)
        } else {
            None  // fixed leg不需要index rounding
        };

        // partition_point找到第一個屬於projected（未來）的flow的位置
        let pred = |flow_observer: &FlowObserver| {
            if include_horizon {
                flow_observer.payment_date() <= horizon
            } else {
                flow_observer.payment_date() < horizon
            }
        };

        let pos = self.flow_oberver_list.partition_point(pred);

        for i in pos..self.flow_oberver_list().len() {
            cash_flows[&self.flow_oberver_list()[i].payment_date()] += self
                .flow_oberver_list()[i]
                .projected_flow(forward_curve_opt.as_ref(), &pricing_condition, flow_rounding_digits_opt, index_rounding_digits_opt);
        }

        cash_flows
    }
}