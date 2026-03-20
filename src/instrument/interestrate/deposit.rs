use std::collections::HashMap;
use std::sync::{
    Arc, 
    RwLock
};

use chrono::NaiveDate;
use serde::Deserialize;

use crate::instrument::interestrate::flowobserver::{
    CapitalizationFlow, 
    FlowObserver
};
use crate::instrument::instrument::{
    CurveFunction, 
    Instrument, 
    InstrumentWithLinearFlows, 
    Position, SimpleInstrument
};
use crate::instrument::interestrate::simpleinterestrateinstrumentgenerator::SimpleInterestRateInstrumentGenerator;
use crate::instrument::leg::legcharacters::{LegCharacters, LegCharactersGenerator};
use crate::instrument::leg::legcharactersgeneratorloader::{
    build_leg_characters_generator,
    InterestRateInstrumentSupports,
    LegJsonProp,
};
use crate::manager::manager::{JsonLoader, ManagerBuilder};
use crate::manager::managererror::{ManagerError, parse_json_value};
use crate::manager::namedobject::NamedJsonObject;
use crate::market::market::Market;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::period::Period;
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


impl SimpleInstrument for Deposit {}


pub struct DepositGenerator {
    profit_and_loss_market: Arc<dyn Market>,
    leg_character_genrator: Arc<dyn LegCharactersGenerator>,
    nominal: RwLock<f64>
}

impl DepositGenerator {
    pub fn new(profit_and_loss_market: Arc<dyn Market>, leg_character_genrator: Arc<dyn LegCharactersGenerator>, nominal: f64) -> Self {
        Self {
            profit_and_loss_market,
            leg_character_genrator,
            nominal: RwLock::new(nominal)
        }
    }

    pub fn leg_character_genrator(&self) -> &Arc<dyn LegCharactersGenerator> {
        &self.leg_character_genrator
    }

    pub fn nominal(&self) -> f64 {
        *self.nominal.read().unwrap()
    }

    pub fn set_nominal(&self, v: f64) {
        *self.nominal.write().unwrap() = v;
    }
}

impl SimpleInterestRateInstrumentGenerator for DepositGenerator {
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
        let leg_characters = self.leg_character_genrator.generate_with_maturity_date(trade_date, maturity_date, start_date_opt)?;
        Ok(Arc::new(Deposit::new(position, self.nominal(), leg_characters, self.profit_and_loss_market.clone())))
    }

    fn generate_with_maturity_tenor(
        &self, 
        position: Position, 
        trade_date: NaiveDate,
        maturity_tenor: Period,
        start_date_opt: Option<NaiveDate>
    ) -> Result<Arc<dyn SimpleInstrument>, String> {
        let leg_characters = self.leg_character_genrator.generate_with_maturity_tenor(trade_date, maturity_tenor, start_date_opt)?;
        Ok(Arc::new(Deposit::new(position, self.nominal(), leg_characters, self.profit_and_loss_market.clone())))
    }
}


// ═════════════════════════════════════════════════════════════════════════════
// DepositGeneratorLoader
// ═════════════════════════════════════════════════════════════════════════════
//
// 設計說明：
//   Leg 定義直接內嵌在 JSON 中（而非透過名稱引用另一個 manager），
//   讓一份 JSON 物件就能完整描述一個 Deposit 產品，不需要上下跳動查閱。
//
//   外部依賴（market / calendar / schedule / day_counter / index）仍以名稱
//   引用，對應到 InterestRateInstrumentSupports 中的各 FrozenManager。
//   這些才是真正跨產品共用的建構件。
//
// JSON 範例（固定利率 Deposit）：
//   {
//     "name": "TWD_3M_FIXED_DEPOSIT",
//     "market": "TWD_MARKET",
//     "nominal": 10000000.0,
//     "leg": {
//       "type": "Fixed",
//       "calendar": "TWD",
//       "schedule_generator": "TWD_3M_SCHED",
//       "day_counter_generator": "ACT365",
//       "compounding": "Simple",
//       "rate": 0.0185
//     }
//   }
//
// JSON 範例（浮動利率 Deposit）：
//   {
//     "name": "USD_3M_FLOATING_DEPOSIT",
//     "market": "USD_MARKET",
//     "nominal": 5000000.0,
//     "leg": {
//       "type": "Floating",
//       "calendar": "USD",
//       "schedule_generator": "USD_3M_SCHED",
//       "day_counter_generator": "ACT360",
//       "compounding": "Simple",
//       "index": "USD_LIBOR_3M",
//       "spread": 0.0
//     }
//   }

fn default_nominal() -> f64 {
    1_000_000.0
}

#[derive(Deserialize)]
struct DepositGeneratorJsonProp {
    market:  String,
    /// 省略時預設 1_000_000.0（對應 [`NominalSetter`] 的慣例預設值）。
    #[serde(default = "default_nominal")]
    nominal: f64,
    /// Leg 定義直接內嵌，不透過獨立的 leg manager 查找。
    leg:     LegJsonProp,
}

/// [`DepositGenerator`] 的 JSON 載入器。
///
/// 實作 [`JsonLoader`]，搭配 [`InterestRateInstrumentSupports`] 使用。
///
/// # 使用方式
/// ```rust
/// let mut builder: ManagerBuilder<DepositGenerator> = ManagerBuilder::new();
/// DepositGeneratorLoader
///     .load_from_reader(&mut builder, "deposits.json", &(market, cal, sched, dcg, idx))?;
/// let frozen: FrozenManager<DepositGenerator> = builder.build();
/// ```
pub struct DepositGeneratorLoader;

impl<'a> JsonLoader<DepositGenerator, InterestRateInstrumentSupports<'a>> for DepositGeneratorLoader {
    fn insert_obj_from_json(
        &self,
        builder: &mut ManagerBuilder<DepositGenerator>,
        json_value: serde_json::Value,
        supports: &InterestRateInstrumentSupports<'a>,
    ) -> Result<(), ManagerError> {
        let named: NamedJsonObject = parse_json_value(json_value.clone())?;
        let prop:  DepositGeneratorJsonProp = parse_json_value(json_value)?;

        let market    = supports.0.get(&prop.market)?;
        let leg_chars = build_leg_characters_generator(prop.leg, supports)?;

        let generator = DepositGenerator::new(market, leg_chars, prop.nominal);
        builder.insert(named.name().to_owned(), Arc::new(generator));
        Ok(())
    }
}