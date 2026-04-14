use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::instrument::{
    CurveFunction, 
    SimpleInstrument
};
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricer::pricer::Pricer;
use crate::pricingcondition::PricingCondition;
use crate::value::npv::NPV;


pub struct SimpleInstrumentPricer;

impl SimpleInstrumentPricer {
    fn market_value_at_horizon(
        &self,
        instrument: &dyn SimpleInstrument,
        market_data: &HashMap<String, Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition
    ) -> Option<NPV> {
        let curve_name_map = instrument.curve_name_map();
        let forward_curve_opt = curve_name_map.
            get(&CurveFunction::PayForward).
            map_or(
                None, 
              |curve_name| market_data.get(curve_name)
            );
        let pay_flows = instrument.projected_pay_flows(forward_curve_opt, pricing_condition);
        let forward_curve_opt = curve_name_map.
            get(&CurveFunction::ReceiveForward).
            map_or(
                None, 
              |curve_name| market_data.get(curve_name)
            );
        let receive_flows = instrument.projected_receive_flows(forward_curve_opt, pricing_condition);
        let discount_curve_opt = curve_name_map.
            get(&CurveFunction::ProfitAndLossDiscount).
            map_or(
                None, 
              |curve_name| market_data.get(curve_name)
            );
        let horizon: NaiveDate = *pricing_condition.horizon();
        let npv_value_opt = discount_curve_opt.map_or(
            None, 
            | discount_curve | Some((pay_flows + receive_flows).npv(discount_curve, Some(horizon)))
        );
        let settlement_currency = instrument.profit_and_loss_market().settlement_currency().clone();
        npv_value_opt.map_or(
            None,
            |npv_value| Some(NPV::new(settlement_currency, npv_value, horizon))
        )
    }
}

impl Pricer<dyn SimpleInstrument, HashMap<String, Arc<dyn InterestRateCurve>>> for SimpleInstrumentPricer {
    fn market_value(
        &self,
        instrument: &dyn SimpleInstrument,
        market_data: &HashMap<String, Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition
    ) -> Option<NPV> {
        let market_value_at_horizon = self.market_value_at_horizon(instrument, market_data, pricing_condition)?;
        let curve_name_map = instrument.curve_name_map();
        let discount_curve = curve_name_map.
            get(&CurveFunction::ProfitAndLossDiscount).
            map_or(
                None, 
              |curve_name| market_data.get(curve_name)
            )?;
        let settlement_date = instrument.profit_and_loss_market().settlement_date(*pricing_condition.horizon());
        let npv_value = market_value_at_horizon.amount() / discount_curve.to_discount_curve().discount(settlement_date);
        let settlement_currency = instrument.profit_and_loss_market().settlement_currency().clone();
        Some(NPV::new(settlement_currency, npv_value, settlement_date))
    }

    fn econ_profit_and_loss(
        &self,
        instrument: &dyn SimpleInstrument,
        market_data: &HashMap<String, Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition
    ) -> Option<NPV> {
        let market_value_at_horizon = self.market_value_at_horizon(instrument, market_data, pricing_condition)?;
        let past_cash_proceeds = instrument.past_receive_flows(pricing_condition) - instrument.past_pay_flows(pricing_condition);
        let econ_pnl_value = market_value_at_horizon.amount() + past_cash_proceeds.sum();
        let settlement_currency = instrument.profit_and_loss_market().settlement_currency().clone();
        Some(NPV::new(settlement_currency, econ_pnl_value, *pricing_condition.horizon()))
    }
}