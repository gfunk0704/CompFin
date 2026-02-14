use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;

use chrono::NaiveDate;

use crate::instrument::instrument::{
    Instrument, 
    Position
};
use crate::instrument::leg::legcharacters::LegCharacters;
use crate::instrument::nominalgenerator::NominalGenerator;  
use crate::instrument::interestrate::flowobserver::{
    CapitalizationFlow,
    FlowObserver
};
use crate::market::market::Market;
use crate::math::round::round;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::value::cashflows::CashFlows;
use crate::value::npv::NPV;

pub struct Deposit {
    position: Position,
    nominal_generator: Rc<dyn NominalGenerator>,
    leg_characters: Rc<dyn LegCharacters>,
    flow_observers: Vec<FlowObserver>,
    capitalization_flows: Vec<CapitalizationFlow>,
    profit_and_loss_market: Rc<dyn Market>,
    curve_name_map: HashMap<String, String>
}

impl Deposit {
    pub fn new(position: Position,
               nominal_generator: Rc<dyn NominalGenerator>,
               leg_characters: Rc<dyn LegCharacters>,
               profit_and_loss_market: Rc<dyn Market>,
               discount_curve: Option<Rc<dyn InterestRateCurve>>,
               forward_curve: Option<Rc<dyn InterestRateCurve>>,
               curve_name_map: HashMap<String, String>) -> Self {

       

        let mut flow_observers: Vec<FlowObserver> = Vec::new(); // To be implemented: create flow observers based on leg_characters and nominal_generator
        let nominals = nominal_generator.generate_nominal(leg_characters.generic_characters().schedule());
        
        for i in 0..leg_characters.generic_characters().schedule().schedule_periods().len() {
            flow_observers.push(FlowObserver::new(leg_characters.clone(), nominals[i].1, i));
        }

        let mut capitalization_flows: Vec<CapitalizationFlow> = Vec::new();

        capitalization_flows.push(
            CapitalizationFlow::new(
            nominal_generator.setter().initial_nominal(),
            leg_characters.generic_characters().schedule().schedule_periods()[0].calculation_period().start_date()
            )
        );

        capitalization_flows.push(
            CapitalizationFlow::new(
                 nominals.last().unwrap().1,
                nominals.last().unwrap().0
            )
        );

        Self {
            position,
            nominal_generator,
            leg_characters,
            flow_observers,
            capitalization_flows,
            profit_and_loss_market,
            discount_curve: RefCell::new(discount_curve),
            forward_curve: RefCell::new(forward_curve),
            curve_name_map
        }
    }

    pub fn nominal_generator(&self) -> &Rc<dyn NominalGenerator> {
        &self.nominal_generator
    }

    pub fn discount_curve(&self) ->Option<Rc<dyn InterestRateCurve>> {
        self.discount_curve.borrow_mut().clone()
    }

    pub fn past_cash_flows(&self, pricing_condition: &PricingCondition) -> CashFlows {
        let op = if *pricing_condition.include_horizon_flow() {
            <NaiveDate as PartialOrd<NaiveDate>>::lt
        } else {
            <NaiveDate as PartialOrd<NaiveDate>>::le
        };

        let settlement_date = self.profit_and_loss_market.settlement_date(*pricing_condition.horizon());
        let cash_flows = self.creat_capitalization_flows(&settlement_date, op);
        let mut upper_bound = self.flow_observers.partition_point(|flow_observer: &FlowObserver| flow_observer.payment_date() < settlement_date);
        
        if !pricing_condition.include_horizon_flow() {
            upper_bound += (self.flow_observers[upper_bound].payment_date() == settlement_date) as usize;
        }

        for flow_observer in self.flow_observers[..upper_bound].iter() {
            let payment_date = flow_observer.payment_date();
            let flow_value = flow_observer.projected_flow(
                    self.forward_curve.borrow().as_ref(),
                    pricing_condition,
                    Some(self.profit_and_loss_market.settlement_currency().digits())
                );
            cash_flows.set_value(&payment_date, flow_value);
        }

        cash_flows
    }
    
    pub fn projection_cash_flows(&self, pricing_condition: &PricingCondition) -> CashFlows {
        let op = if *pricing_condition.include_horizon_flow() {
            <NaiveDate as PartialOrd<NaiveDate>>::ge
        } else {
            <NaiveDate as PartialOrd<NaiveDate>>::gt
        };

        let settlement_date = self.profit_and_loss_market.settlement_date(*pricing_condition.horizon());
        let cash_flows = self.creat_capitalization_flows(&settlement_date, op);
        let settlement_date = self.profit_and_loss_market.settlement_date(*pricing_condition.horizon());
        let mut start_index = self.flow_observers.partition_point(|flow_observer: &FlowObserver| flow_observer.payment_date() < settlement_date);
        if !pricing_condition.include_horizon_flow() {
            start_index += (self.flow_observers[start_index].payment_date() == settlement_date) as usize;
        }

        for flow_observer in self.flow_observers[start_index..].iter() {
            let payment_date = flow_observer.payment_date();
            let flow_value = flow_observer.projected_flow(
                    self.forward_curve.borrow().as_ref(),
                    pricing_condition,
                    Some(self.profit_and_loss_market.settlement_currency().digits())
                );
            cash_flows.set_value(&payment_date, flow_value);
        }

        cash_flows
    }

    pub fn creat_capitalization_flows(&self, 
                                  settlement_date: &NaiveDate,
                                  op: fn(&NaiveDate, &NaiveDate) -> bool) -> CashFlows {

        let cash_flows = CashFlows::new();

        for capitalization_flow in self.capitalization_flows.iter() {
            let payment_date = capitalization_flow.payment_date();
            if op(&payment_date, settlement_date)  {
                let flow_value = capitalization_flow.amount();
                cash_flows.set_value(&payment_date, flow_value);
            }
        }

        cash_flows
    }

    fn market_value_impl(&self, 
                         pricing_condition: &PricingCondition,
                         settlement_date_opt: Option<NaiveDate>) -> f64 {
        let flows = self.projection_cash_flows(pricing_condition);
        let discount_curve = self.discount_curve.borrow().as_ref().unwrap().clone();
        let mut mv: f64 = flows.npv(
            &discount_curve,
            settlement_date_opt
        );

        if pricing_condition.dacimal_rounding().deterministic_flow() {
            mv = round(mv, self.profit_and_loss_market.settlement_currency().digits());
        }

        mv
    }
}

impl Instrument for Deposit {
    fn is_nonlinear(&self) -> bool {
        false
    }

    fn max_date(&self) -> NaiveDate {
        self.leg_characters.max_date()
    }

    fn position(&self) -> Position {
        self.position
    }

    fn profit_and_loss_market(&self) -> Rc<dyn Market> {
        self.profit_and_loss_market.clone()
    }

    fn curve_name_map(&self) -> &HashMap<String, String> {
        &self.curve_name_map
    }

    fn curve_map(&self) -> HashMap<String, Option<Rc<dyn InterestRateCurve>>> {
        let mut curve_map: HashMap<String, Option<Rc<dyn InterestRateCurve>>> = HashMap::new();
        curve_map.insert(String::from_str("discount").unwrap(), self.discount_curve.borrow_mut().clone());
        curve_map.insert(String::from_str("forward").unwrap(), self.forward_curve.borrow_mut().clone());
        curve_map
    }

    fn set_curve_by_name(&self, curve_map: &HashMap<String, Option<Rc<dyn InterestRateCurve>>>) -> () {
        for (curve_function, curve_name) in self.curve_name_map.iter() {
            let curve_opt = curve_map.get(curve_name);
            if curve_opt.is_some() {
                match curve_function.as_str() {
                    "discount" => {
                        self.discount_curve.replace(curve_opt.unwrap().clone());
                    },
                    "forward" => {
                        self.forward_curve.replace(curve_opt.unwrap().clone());
                    },
                    _ => {}
                }
            }
        }
    }

    fn set_curve_by_function(&self, curve_map: &HashMap<String, Option<Rc<dyn InterestRateCurve>>>) -> () {
        for curve_function in self.curve_name_map.keys() {
            let curve_opt = curve_map.get(curve_function);
            if curve_opt.is_some() {
                match curve_function.as_str() {
                    "discount" => {
                        self.discount_curve.replace(curve_opt.unwrap().clone());
                    },
                    "forward" => {
                        self.forward_curve.replace(curve_opt.unwrap().clone());
                    },
                    _ => {}
                }
            }
        }
    }
    
    fn market_value(self, pricing_condition: &PricingCondition) -> NPV {
        let settlement_date = self.profit_and_loss_market.settlement_date(*pricing_condition.horizon());
        let mv = self.market_value_impl(pricing_condition, Some(settlement_date));
        NPV::new(self.profit_and_loss_market.settlement_currency().clone(), mv, settlement_date)
    }
    
    fn profit_and_loss(&self, pricing_condition: &PricingCondition) -> NPV {
        let past_cash_value = self.past_cash_flows(pricing_condition).sum();
        let profit_and_loss_value: f64 = self.market_value_impl(pricing_condition, None) + past_cash_value;
        NPV::new(self.profit_and_loss_market.settlement_currency().clone(), profit_and_loss_value, *pricing_condition.horizon())
    }
}


