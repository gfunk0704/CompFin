use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::instrument::instrument::{
    CurveFunction,
    Instrument,
    InstrumentWithLinearFlows,
    Position, SimpleInstrument,
};
use crate::instrument::interestrate::flowobserver::FlowObserver;
use crate::instrument::interestrate::simpleinterestrateinstrumentgenerator::{
    build_leg_characters_generator,
    InterestRateInstrumentSupports,
    LegJsonProp,
    SimpleInterestRateInstrumentGenerator,
};
use crate::instrument::leg::legcharacters::{LegCharacters, LegCharactersGenerator};
use crate::instrument::nominalgenerator::{
    build_nominal_generator,
    NominalGenerator,
    NominalGeneratorJsonProp,
};
use crate::manager::manager::{JsonLoader, ManagerBuilder};
use crate::manager::managererror::{ManagerError, parse_json_value};
use crate::manager::namedobject::NamedJsonObject;
use crate::market::market::Market;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::value::cashflows::CashFlows;


pub struct InterestRateSwap {
    position: Position,
    profit_and_loss_market: Arc<dyn Market>,
    pay_leg_characters: Arc<dyn LegCharacters>,
    pay_leg_flow_observer_list: Vec<FlowObserver>,
    receive_leg_characters: Arc<dyn LegCharacters>,
    receive_leg_flow_observer_list: Vec<FlowObserver>,
    curve_name_map: HashMap<CurveFunction, String>,
}

impl InterestRateSwap {
    pub fn new(
        position: Position,
        profit_and_loss_market: Arc<dyn Market>,
        pay_leg_characters: Arc<dyn LegCharacters>,
        pay_leg_nominals: Vec<f64>,
        receive_leg_characters: Arc<dyn LegCharacters>,
        receive_leg_nominals: Vec<f64>,
    ) -> Self {
        let pay_leg_flow_observer_list = Self::build_flow_observer_list(
            &pay_leg_characters,
            pay_leg_nominals,
        );
        let receive_leg_flow_observer_list = Self::build_flow_observer_list(
            &receive_leg_characters,
            receive_leg_nominals,
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
            pay_leg_flow_observer_list,
            receive_leg_characters,
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

    fn build_flow_observer_list(
        leg_characters: &Arc<dyn LegCharacters>,
        nominals: Vec<f64>,
    ) -> Vec<FlowObserver> {
        nominals
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

impl SimpleInstrument for InterestRateSwap {}


pub struct InterestRateSwapGenerator {
    profit_and_loss_market: Arc<dyn Market>,
    pay_leg_character_genrator: Arc<dyn LegCharactersGenerator>,
    pay_leg_nominal_generator: Arc<dyn NominalGenerator>,
    receive_leg_character_genrator: Arc<dyn LegCharactersGenerator>,
    receive_leg_nominal_generator: Arc<dyn NominalGenerator>,
}

impl InterestRateSwapGenerator {
    pub fn new(
        profit_and_loss_market: Arc<dyn Market>,
        pay_leg_character_genrator: Arc<dyn LegCharactersGenerator>,
        pay_leg_nominal_generator: Arc<dyn NominalGenerator>,
        receive_leg_character_genrator: Arc<dyn LegCharactersGenerator>,
        receive_leg_nominal_generator: Arc<dyn NominalGenerator>,
    ) -> Self {
        Self {
            profit_and_loss_market,
            pay_leg_character_genrator,
            pay_leg_nominal_generator,
            receive_leg_character_genrator,
            receive_leg_nominal_generator,
        }
    }

    pub fn pay_leg_character_genrator(&self) -> &Arc<dyn LegCharactersGenerator> {
        &self.pay_leg_character_genrator
    }

    pub fn receive_leg_character_genrator(&self) -> &Arc<dyn LegCharactersGenerator> {
        &self.receive_leg_character_genrator
    }

    pub fn pay_leg_nominal_generator(&self) -> &Arc<dyn NominalGenerator> {
        &self.pay_leg_nominal_generator
    }

    pub fn receive_leg_nominal_generator(&self) -> &Arc<dyn NominalGenerator> {
        &self.receive_leg_nominal_generator
    }
}


impl SimpleInterestRateInstrumentGenerator for InterestRateSwapGenerator {
    fn profit_and_loss_market(&self) -> &Arc<dyn Market> {
        &self.profit_and_loss_market
    }

    fn generate_with_maturity_date(
        &self, 
        position: Position, 
        trade_date: NaiveDate,
        maturity_date: NaiveDate,
        start_date_opt: Option<NaiveDate>
    ) -> Result<Arc<dyn SimpleInstrument>, String> {
        let pay_leg_characters = self.pay_leg_character_genrator.generate_with_maturity_date(
            trade_date,
            maturity_date,
            start_date_opt,
        )?;
        
        let receive_leg_characters = self.receive_leg_character_genrator.generate_with_maturity_date(
            trade_date,
            maturity_date,
            start_date_opt,
        )?;

        let pay_leg_nominals = self.pay_leg_nominal_generator.generate_nominal(
            pay_leg_characters.generic_characters().schedule()
        );

        let receive_leg_nominals = self.receive_leg_nominal_generator.generate_nominal(
            receive_leg_characters.generic_characters().schedule()
        );

        Ok(Arc::new(InterestRateSwap::new(
            position,
            self.profit_and_loss_market.clone(),
            pay_leg_characters,
            pay_leg_nominals,
            receive_leg_characters,
            receive_leg_nominals,
        )))
    }

    fn generate_with_maturity_tenor(
            &self, 
            position: Position, 
            trade_date: NaiveDate,
            maturity_tenor: crate::time::period::Period,
            start_date_opt: Option<NaiveDate>
        ) -> Result<Arc<dyn SimpleInstrument>, String> {
        let pay_leg_characters = self.pay_leg_character_genrator.generate_with_maturity_tenor(
            trade_date,
            maturity_tenor,
            start_date_opt,
        )?;

        let receive_leg_characters = self.receive_leg_character_genrator.generate_with_maturity_tenor(
            trade_date,
            maturity_tenor,
            start_date_opt,
        )?;

        let pay_leg_nominals = self.pay_leg_nominal_generator.generate_nominal(
            pay_leg_characters.generic_characters().schedule()
        );

        let receive_leg_nominals = self.receive_leg_nominal_generator.generate_nominal(
            receive_leg_characters.generic_characters().schedule()
        );

        Ok(Arc::new(InterestRateSwap::new(
            position,
            self.profit_and_loss_market.clone(),
            pay_leg_characters,
            pay_leg_nominals,
            receive_leg_characters,
            receive_leg_nominals,
        )))
    }
}


// ═════════════════════════════════════════════════════════════════════════════
// InterestRateSwapGeneratorLoader
// ═════════════════════════════════════════════════════════════════════════════
//
// 設計說明：
//   Pay / receive 兩條腿直接以 LegJsonProp 內嵌在 JSON 中，
//   閱讀一份 JSON 就能理解整筆 IRS 的完整結構。
//   NominalGeneratorJsonProp / build_nominal_generator 定義於
//   simpleinterestrateinstrumentgenerator，Bond 等其他產品亦可共用。
//
// JSON 範例（固定 vs. 浮動，固定名目本金）：
//   {
//     "name": "TWD_IRS_3Mv6M",
//     "market": "TWD_MARKET",
//     "pay_leg": {
//       "type": "Fixed",
//       "calendar": "TWD",
//       "schedule_generator": "TWD_3M_SCHED",
//       "day_counter_generator": "ACT365",
//       "compounding": "Simple",
//       "rate": 0.0200
//     },
//     "pay_leg_nominal":  { "type": "Fixed", "initial_nominal": 100000000.0 },
//     "receive_leg": {
//       "type": "Floating",
//       "calendar": "TWD",
//       "schedule_generator": "TWD_6M_SCHED",
//       "day_counter_generator": "ACT365",
//       "compounding": "Simple",
//       "index": "TWD_LIBOR_6M",
//       "spread": 0.0
//     },
//     "receive_leg_nominal": { "type": "Fixed", "initial_nominal": 100000000.0 }
//   }
//
// JSON 範例（遞增名目本金的 pay leg）：
//   "pay_leg_nominal": {
//     "type": "Accreting",
//     "initial_nominal": 100000000.0,
//     "rate": 0.03,
//     "day_counter_generator": "ACT365",
//     "compounding": "Annual"
//   }

#[derive(Deserialize)]
struct InterestRateSwapGeneratorJsonProp {
    market:               String,
    pay_leg:              LegJsonProp,
    pay_leg_nominal:      NominalGeneratorJsonProp,
    receive_leg:          LegJsonProp,
    receive_leg_nominal:  NominalGeneratorJsonProp,
}

/// [`InterestRateSwapGenerator`] 的 JSON 載入器。
///
/// 實作 [`JsonLoader`]，搭配 [`InterestRateInstrumentSupports`] 使用。
///
/// # 使用方式
/// ```rust
/// let mut builder: ManagerBuilder<InterestRateSwapGenerator> = ManagerBuilder::new();
/// InterestRateSwapGeneratorLoader
///     .load_from_reader(&mut builder, "irs.json", &(market, cal, sched, dcg, idx))?;
/// let frozen: FrozenManager<InterestRateSwapGenerator> = builder.build();
/// ```
pub struct InterestRateSwapGeneratorLoader;

impl<'a> JsonLoader<InterestRateSwapGenerator, InterestRateInstrumentSupports<'a>>
    for InterestRateSwapGeneratorLoader
{
    fn insert_obj_from_json(
        &self,
        builder: &mut ManagerBuilder<InterestRateSwapGenerator>,
        json_value: serde_json::Value,
        supports: &InterestRateInstrumentSupports<'a>,
    ) -> Result<(), ManagerError> {
        let named: NamedJsonObject = parse_json_value(json_value.clone())?;
        let prop:  InterestRateSwapGeneratorJsonProp = parse_json_value(json_value)?;

        let market       = supports.0.get(&prop.market)?;
        let pay_leg      = build_leg_characters_generator(prop.pay_leg,     supports)?;
        let receive_leg  = build_leg_characters_generator(prop.receive_leg, supports)?;
        let pay_nominal  = build_nominal_generator(prop.pay_leg_nominal,     supports.3)?;
        let recv_nominal = build_nominal_generator(prop.receive_leg_nominal,  supports.3)?;

        let generator = InterestRateSwapGenerator::new(
            market, pay_leg, pay_nominal, receive_leg, recv_nominal,
        );
        builder.insert(named.name().to_owned(), Arc::new(generator));
        Ok(())
    }
}