use std::sync::Arc;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::instrument::instrument::{Position, SimpleInstrument};
use crate::instrument::leg::fixedratelegcharacters::FixedRateLegCharactersGenerator;
use crate::instrument::leg::fixingratecalculator::fixingratecalculator::FixingRateCalculatorGenerator;
use crate::instrument::leg::fixingratecalculator::termratecalculator::{StubRateConvention, TermRateCalculatorGenerator};
use crate::instrument::leg::floatingratelegcharacters::FloatingRateLegCharactersGenerator;
use crate::instrument::leg::legcharacters::{LegCharactersGenerator, LegCharactersSetter};
use crate::interestrate::compounding::Compounding;
use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::manager::manager::FrozenManager;
use crate::manager::managererror::ManagerError;
use crate::market::market::Market;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::period::Period;
use crate::time::schedule::schedule::ScheduleGenerator;


// ─────────────────────────────────────────────────────────────────────────────
// SimpleInterestRateInstrumentGenerator
// ─────────────────────────────────────────────────────────────────────────────

pub trait SimpleInterestRateInstrumentGenerator {
    fn profit_and_loss_market(&self) -> &Arc<dyn Market>;

    fn generate_with_maturity_date(
        &self, 
        position: Position, 
        trade_date: NaiveDate,
        maturity_date: NaiveDate,
        start_date_opt: Option<NaiveDate>
    ) -> Result<Arc<dyn SimpleInstrument>, String>;

    fn generate_with_maturity_tenor(
        &self, 
        position: Position, 
        trade_date: NaiveDate,
        maturity_tenor: Period,
        start_date_opt: Option<NaiveDate>
    ) -> Result<Arc<dyn SimpleInstrument>, String>;
}


// ─────────────────────────────────────────────────────────────────────────────
// 共用的 Supports 型別
// ─────────────────────────────────────────────────────────────────────────────
//
// 所有 interest rate instrument generator loader 共用相同的外部依賴集合，
// 集中在此定義，避免各個 loader 重複宣告。
//
// 元組欄位：
//   .0  FrozenManager<dyn Market>
//         — P&L market 查找（透過 JSON 中的 "market" 鍵名）
//   .1  FrozenManager<dyn HolidayCalendar + Send + Sync>
//         — calendar 查找（leg 定義中的 calendar / fixing_calendar / payment_calendar）
//   .2  FrozenManager<ScheduleGenerator>
//         — schedule 產生器查找（leg 定義中的 schedule_generator）
//   .3  FrozenManager<DayCounterGenerator>
//         — day counter 產生器查找（leg 定義中的 day_counter_generator；
//           IRS 的 AccretingNominalGenerator 也用此欄位）
//   .4  FrozenManager<dyn InterestRateIndex + Send + Sync>
//         — floating leg 的 index 查找（leg 定義中的 "index"，僅 Floating 型別使用）

pub type InterestRateInstrumentSupports<'a> = (
    &'a FrozenManager<dyn Market>,
    &'a FrozenManager<dyn HolidayCalendar + Send + Sync>,
    &'a FrozenManager<ScheduleGenerator>,
    &'a FrozenManager<DayCounterGenerator>,
    &'a FrozenManager<dyn InterestRateIndex + Send + Sync>,
);


// ─────────────────────────────────────────────────────────────────────────────
// LegJsonProp
// ─────────────────────────────────────────────────────────────────────────────
//
// Leg 定義直接內嵌在 instrument generator 的 JSON 中，不獨立存一個 manager。
// 這反映 Murex 的設計哲學：LegCharactersGenerator 是 instrument 的實作細節，
// 不是值得跨產品共用的業務實體。
//
// 以 "type" 欄位區分 Fixed / Floating 兩種型別。
// 只有真正可重用的原始建構件（calendar / schedule / day_counter / index）
// 才以名稱引用，從對應的 FrozenManager 查找。
//
// JSON 範例（固定利率 leg）：
//   {
//     "type": "Fixed",
//     "calendar": "TWD",
//     "schedule_generator": "TWD_3M_SCHED",
//     "day_counter_generator": "ACT365",
//     "compounding": "Simple",
//     "rate": 0.02
//   }
//
// JSON 範例（浮動利率 leg，TermRate index）：
//   {
//     "type": "Floating",
//     "calendar": "TWD",
//     "schedule_generator": "TWD_6M_SCHED",
//     "day_counter_generator": "ACT365",
//     "compounding": "Simple",
//     "index": "TWD_LIBOR_6M",
//     "spread": 0.0005,
//     "leverage": 1.0
//   }
//
// 注意：CompoundingRate index（SOFR / SONIA）目前不在此型別中，
// 因為 CompoundingRateIndexCalculatorGenerator 需要具體的 Arc<CompoundingRateIndex>
// 而非 Arc<dyn InterestRateIndex>，應另行設計獨立的 loader 搭配額外的 supports 欄位。

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum LegJsonProp {
    /// 固定利率 leg。`rate` 讀入 [`LegCharactersSetter`] 的 `fixed_rate`。
    Fixed {
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
        rate: f64,
    },

    /// 浮動利率 leg（TermRate index）。
    /// `index` 對應到 `InterestRateInstrumentSupports.4` 的鍵名。
    Floating {
        calendar: String,
        #[serde(default)]
        fixing_calendar: Option<String>,
        #[serde(default)]
        payment_calendar: Option<String>,
        schedule_generator: String,
        day_counter_generator: String,
        compounding: Compounding,
        index: String,
        #[serde(default)]
        spread: f64,
        #[serde(default = "default_leverage")]
        leverage: f64,
        /// Stub period 的歷史 fixing 計算慣例。若省略，預設為 `Straight`。
        #[serde(default)]
        stub_rate_convention: StubRateConvention,
    },
}

fn default_leverage() -> f64 {
    1.0
}


// ─────────────────────────────────────────────────────────────────────────────
// build_leg_characters_generator
// ─────────────────────────────────────────────────────────────────────────────

/// [`LegJsonProp`] 轉換為 `Arc<dyn LegCharactersGenerator>` 的共用工廠函式。
///
/// 由 `DepositGeneratorLoader` 與 `InterestRateSwapGeneratorLoader` 共用，
/// 避免重複的 calendar / schedule / day_counter 查找邏輯。
pub fn build_leg_characters_generator(
    prop: LegJsonProp,
    supports: &InterestRateInstrumentSupports,
) -> Result<Arc<dyn LegCharactersGenerator>, ManagerError> {
    match prop {
        LegJsonProp::Fixed {
            calendar,
            fixing_calendar,
            payment_calendar,
            schedule_generator,
            day_counter_generator,
            compounding,
            rate,
        } => {
            let cal     = supports.1.get(&calendar)?;
            let fix_cal = resolve_opt_calendar(&fixing_calendar, &cal, supports)?;
            let pay_cal = resolve_opt_calendar(&payment_calendar, &cal, supports)?;
            let sched   = supports.2.get(&schedule_generator)?;
            let dcg     = supports.3.get(&day_counter_generator)?;

            let mut setter = LegCharactersSetter::new();
            setter.set_fixed_rate(rate);

            Ok(Arc::new(FixedRateLegCharactersGenerator::new(
                cal, fix_cal, pay_cal, sched, dcg, compounding, setter,
            )))
        }

        LegJsonProp::Floating {
            calendar,
            fixing_calendar,
            payment_calendar,
            schedule_generator,
            day_counter_generator,
            compounding,
            index,
            spread,
            leverage,
            stub_rate_convention,
        } => {
            let cal     = supports.1.get(&calendar)?;
            let fix_cal = resolve_opt_calendar(&fixing_calendar, &cal, supports)?;
            let pay_cal = resolve_opt_calendar(&payment_calendar, &cal, supports)?;
            let sched   = supports.2.get(&schedule_generator)?;
            let dcg     = supports.3.get(&day_counter_generator)?;
            let idx     = supports.4.get(&index)?;

            let mut setter = LegCharactersSetter::new();
            setter.set_spread(spread);
            setter.set_leverage(leverage);

            // TermRateCalculatorGenerator 與 FloatingRateLegCharactersGenerator
            // 各自持有一份 index 的 Arc clone（cost: 單次原子遞增，可忽略）
            let calc_gen: Arc<dyn FixingRateCalculatorGenerator> = Arc::new(
                TermRateCalculatorGenerator::new(idx.clone(), stub_rate_convention),
            );

            Ok(Arc::new(FloatingRateLegCharactersGenerator::new(
                cal, fix_cal, pay_cal, sched, dcg, compounding, setter, idx, calc_gen,
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