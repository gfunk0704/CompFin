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
    Position, 
    SimpleInstrumentGenerator
};
use crate::instrument::leg::legcharacters::LegCharacters;
use crate::market::market::Market;
use crate::time::schedule;


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

        Self {
            position,
            nominal,
            leg_characters,
            profit_and_loss_market,
            capitalization_flow_list,
            flow_oberver_list
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

    fn curve_name_map(&self) -> &HashMap<CurveFunction, String>;

    fn is_linear(&self) -> bool {
        true
    }
}