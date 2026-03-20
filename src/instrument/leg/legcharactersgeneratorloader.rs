// ── legcharactersgeneratorloader.rs ─────────────────────────────────────────
//
// LegCharactersGenerator 的 JSON 載入支援。
//
// 設計說明：
//   Leg 定義直接內嵌在 instrument generator 的 JSON 中，不獨立存一個 manager。
//   這反映 LegCharactersGenerator 是 instrument 的實作細節，而非跨產品共用的業務實體。
//   只有真正可重用的原始建構件（calendar / schedule / day_counter / index）
//   才以名稱引用，從對應的 FrozenManager 查找。
//
//   LegJsonProp 使用 inner struct 模式（而非直接在 enum variant 欄位上標注 serde 屬性），
//   原因是 serde 的 #[serde(tag = "...")] 內部標記 enum 不支援 variant 欄位層級的
//   #[serde(default)]，拆出獨立 struct 可讓 default 屬性正常運作。

use std::sync::Arc;

use serde::Deserialize;

use crate::instrument::leg::fixedratelegcharacters::FixedRateLegCharactersGenerator;
use crate::instrument::leg::fixingratecalculator::fixingratecalculator::FixingRateCalculatorGenerator;
use crate::instrument::leg::fixingratecalculator::termratecalculator::{
    StubRateConvention,
    TermRateCalculatorGenerator,
};
use crate::instrument::leg::floatingratelegcharacters::FloatingRateLegCharactersGenerator;
use crate::instrument::leg::legcharacters::{LegCharactersGenerator, LegCharactersSetter};
use crate::interestrate::compounding::Compounding;
use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::manager::manager::FrozenManager;
use crate::manager::managererror::ManagerError;
use crate::market::market::Market;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::schedule::schedule::ScheduleGenerator;


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateInstrumentSupports
// ─────────────────────────────────────────────────────────────────────────────
//
// 所有利率商品 generator loader 共用的外部依賴集合。
//
// 元組欄位：
//   .0  FrozenManager<dyn Market>
//         — P&L market 查找
//   .1  FrozenManager<dyn HolidayCalendar + Send + Sync>
//         — calendar 查找（leg 定義中的 calendar / fixing_calendar / payment_calendar）
//   .2  FrozenManager<ScheduleGenerator>
//         — schedule 產生器查找
//   .3  FrozenManager<DayCounterGenerator>
//         — day counter 產生器查找；build_nominal_generator 呼叫端需自行取出 supports.3 傳入
//   .4  FrozenManager<dyn InterestRateIndex + Send + Sync>
//         — floating leg 的 index 查找（僅 Floating 型別使用）

pub type InterestRateInstrumentSupports<'a> = (
    &'a FrozenManager<dyn Market>,
    &'a FrozenManager<dyn HolidayCalendar + Send + Sync>,
    &'a FrozenManager<ScheduleGenerator>,
    &'a FrozenManager<DayCounterGenerator>,
    &'a FrozenManager<dyn InterestRateIndex + Send + Sync>,
);


// ─────────────────────────────────────────────────────────────────────────────
// LegJsonProp inner structs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FixedLegJsonProp {
    calendar: String,
    /// 若省略，使用與 `calendar` 相同的 calendar。
    #[serde(default)]
    fixing_calendar: Option<String>,
    /// 若省略，使用與 `calendar` 相同的 calendar。
    #[serde(default)]
    payment_calendar: Option<String>,
    schedule_generator: String,
    day_counter_generator: String,
    compounding: Compounding,
    /// 省略時預設 0.0（對應 [`LegCharactersSetter`] 的預設值）。
    #[serde(default)]
    rate: f64,
}

#[derive(Deserialize)]
struct FloatingLegJsonProp {
    calendar: String,
    /// 若省略，使用與 `calendar` 相同的 calendar。
    #[serde(default)]
    fixing_calendar: Option<String>,
    /// 若省略，使用與 `calendar` 相同的 calendar。
    #[serde(default)]
    payment_calendar: Option<String>,
    schedule_generator: String,
    day_counter_generator: String,
    compounding: Compounding,
    index: String,
    /// 省略時預設 0.0（對應 [`LegCharactersSetter`] 的預設值）。
    #[serde(default)]
    spread: f64,
    /// 省略時預設 1.0（對應 [`LegCharactersSetter`] 的預設值）。
    #[serde(default = "default_leverage")]
    leverage: f64,
    /// Stub period 的歷史 fixing 計算慣例。省略時預設 `Straight`。
    #[serde(default)]
    stub_rate_convention: StubRateConvention,
}

fn default_leverage() -> f64 {
    1.0
}


// ─────────────────────────────────────────────────────────────────────────────
// LegJsonProp
// ─────────────────────────────────────────────────────────────────────────────
//
// JSON 範例（固定利率 leg，省略 rate 使用預設值 0.0）：
//   {
//     "type": "Fixed",
//     "calendar": "TWD",
//     "schedule_generator": "TWD_3M_SCHED",
//     "day_counter_generator": "ACT365",
//     "compounding": "Simple"
//   }
//
// JSON 範例（浮動利率 leg）：
//   {
//     "type": "Floating",
//     "calendar": "TWD",
//     "schedule_generator": "TWD_6M_SCHED",
//     "day_counter_generator": "ACT365",
//     "compounding": "Simple",
//     "index": "TWD_LIBOR_6M",
//     "spread": 0.0005
//   }
//
// 注意：CompoundingRate index（SOFR / SONIA）目前不在此型別中，
// 因為 CompoundingRateIndexCalculatorGenerator 需要具體的 Arc<CompoundingRateIndex>
// 而非 Arc<dyn InterestRateIndex>，應另行設計獨立的 loader 搭配額外的 supports 欄位。

/// Leg 定義的內嵌 JSON 型別。以 `type` 欄位區分 Fixed / Floating 兩種 leg。
#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum LegJsonProp {
    Fixed(FixedLegJsonProp),
    Floating(FloatingLegJsonProp),
}


// ─────────────────────────────────────────────────────────────────────────────
// build_leg_characters_generator
// ─────────────────────────────────────────────────────────────────────────────

/// [`LegJsonProp`] 轉換為 `Arc<dyn LegCharactersGenerator>` 的共用工廠函式。
///
/// 由所有利率商品的 loader（`DepositGeneratorLoader`、`InterestRateSwapGeneratorLoader` 等）共用。
pub fn build_leg_characters_generator(
    prop: LegJsonProp,
    supports: &InterestRateInstrumentSupports,
) -> Result<Arc<dyn LegCharactersGenerator>, ManagerError> {
    match prop {
        LegJsonProp::Fixed(p) => {
            let cal     = supports.1.get(&p.calendar)?;
            let fix_cal = resolve_opt_calendar(&p.fixing_calendar, &cal, supports)?;
            let pay_cal = resolve_opt_calendar(&p.payment_calendar, &cal, supports)?;
            let sched   = supports.2.get(&p.schedule_generator)?;
            let dcg     = supports.3.get(&p.day_counter_generator)?;

            let mut setter = LegCharactersSetter::new();
            setter.set_fixed_rate(p.rate);

            Ok(Arc::new(FixedRateLegCharactersGenerator::new(
                cal, fix_cal, pay_cal, sched, dcg, p.compounding, setter,
            )))
        }

        LegJsonProp::Floating(p) => {
            let cal     = supports.1.get(&p.calendar)?;
            let fix_cal = resolve_opt_calendar(&p.fixing_calendar, &cal, supports)?;
            let pay_cal = resolve_opt_calendar(&p.payment_calendar, &cal, supports)?;
            let sched   = supports.2.get(&p.schedule_generator)?;
            let dcg     = supports.3.get(&p.day_counter_generator)?;
            let idx     = supports.4.get(&p.index)?;

            let mut setter = LegCharactersSetter::new();
            setter.set_spread(p.spread);
            setter.set_leverage(p.leverage);

            // TermRateCalculatorGenerator 與 FloatingRateLegCharactersGenerator
            // 各自持有一份 index 的 Arc clone（cost: 單次原子遞增，可忽略）
            let calc_gen: Arc<dyn FixingRateCalculatorGenerator> = Arc::new(
                TermRateCalculatorGenerator::new(idx.clone(), p.stub_rate_convention),
            );

            Ok(Arc::new(FloatingRateLegCharactersGenerator::new(
                cal, fix_cal, pay_cal, sched, dcg, p.compounding, setter, idx, calc_gen,
            )))
        }
    }
}

/// `fixing_calendar` / `payment_calendar` 省略時，回退到主 calendar。
fn resolve_opt_calendar(
    name_opt: &Option<String>,
    default_cal: &Arc<dyn HolidayCalendar + Send + Sync>,
    supports: &InterestRateInstrumentSupports,
) -> Result<Arc<dyn HolidayCalendar + Send + Sync>, ManagerError> {
    match name_opt {
        Some(name) => supports.1.get(name),
        None       => Ok(Arc::clone(default_cal)),
    }
}
