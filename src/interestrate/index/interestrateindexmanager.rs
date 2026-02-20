// ── interestrateindexmanager.rs ──────────────────────────────────────────────
//
// 遷移至新的兩階段 Manager 架構（ManagerBuilder → FrozenManager）。
//
// # 重大變更
//
// 舊版使用 `RefCell<HashMap<String, Rc<dyn InterestRateIndex>>>` + 舊版 `IManager`。
// 新版：
//   - 全面改用 Arc（移除 Rc）
//   - 實作新版 IManager<dyn InterestRateIndex + Send + Sync, ...>
//   - Supports 型別改用 FrozenManager（而非舊的 Manager<Rc<...>>）
//   - 加入 past_fixings 欄位（舊版遺漏）
//   - 加入 CompoundingRateIndex 的 JSON 載入路徑

use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::interestrate::compounding::Compounding;
use crate::interestrate::index::compoundingrateindex::CompoundingRateIndex;
use crate::interestrate::index::interestrateindex::{InterestRateIndex, InterestRateIndexType};
use crate::interestrate::index::termrateindex::TermRateIndex;
use crate::manager::manager::{IManager, ManagerBuilder, FrozenManager};
use crate::manager::managererror::ManagerError;
use crate::manager::namedobject::NamedJsonObject;
use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::period::{ParsePeriodError, Period};


// ─────────────────────────────────────────────────────────────────────────────
// JSON props
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TermRateIndexJsonProp {
    reference_curve_name: String,
    start_lag: u32,
    adjuster: BusinessDayAdjuster,
    tenor: String,
    calendar: String,
    day_counter_generator: String,
    compounding: Compounding,
    /// 歷史 fixing 資料，格式：{"2024-01-15": 0.0531, ...}
    /// 若無歷史資料可省略（預設空 HashMap）
    #[serde(default)]
    past_fixings: HashMap<NaiveDate, f64>,
}

#[derive(Deserialize)]
struct CompoundingRateIndexJsonProp {
    reference_curve_name: String,
    start_lag: u32,
    adjuster: BusinessDayAdjuster,
    tenor: String,
    calendar: String,
    day_counter_generator: String,
    result_compounding: Compounding,
    /// 每日 overnight past fixings（key = 業務日，value = overnight rate）
    #[serde(default)]
    daily_past_fixings: HashMap<NaiveDate, f64>,
}

#[derive(Deserialize)]
struct InterestRateIndexJsonProp {
    index_type: InterestRateIndexType,
    props: serde_json::Value,
}


// ─────────────────────────────────────────────────────────────────────────────
// 工廠函式
// ─────────────────────────────────────────────────────────────────────────────

// FrozenManager 要求 V: ?Sized + Send + Sync。
// `dyn HolidayCalendar` 已有 Send+Sync supertrait，故可直接使用；
// DayCounterGenerator 是具體型別，不需要額外標註。
type Supports<'a> = (
    &'a FrozenManager<dyn HolidayCalendar + Send + Sync>,
    &'a FrozenManager<DayCounterGenerator>,
);

fn parse_period(s: String) -> Result<Period, ManagerError> {
    Period::parse(s).map_err(ManagerError::TenorParseError)
}

fn build_term_rate_index(
    json_value: serde_json::Value,
    supports: &Supports,
) -> Result<Arc<dyn InterestRateIndex + Send + Sync>, ManagerError> {
    let p: TermRateIndexJsonProp =
        ManagerError::from_json_or_json_parse_error(json_value)?;

    let tenor = parse_period(p.tenor)?;
    let calendar = supports.0.get(&p.calendar)?;
    let dcg = supports.1.get(&p.day_counter_generator)?;
    let day_counter = dcg.generate(None)
        .map_err(ManagerError::DayCounterGenerationError)?;

    Ok(Arc::new(TermRateIndex::new(
        p.reference_curve_name,
        p.start_lag,
        p.adjuster,
        tenor,
        calendar,
        day_counter,
        p.compounding,
        p.past_fixings,
    )))
}

fn build_compounding_rate_index(
    json_value: serde_json::Value,
    supports: &Supports,
) -> Result<Arc<dyn InterestRateIndex + Send + Sync>, ManagerError> {
    let p: CompoundingRateIndexJsonProp =
        ManagerError::from_json_or_json_parse_error(json_value)?;

    let tenor = parse_period(p.tenor)?;
    let calendar = supports.0.get(&p.calendar)?;
    let dcg = supports.1.get(&p.day_counter_generator)?;
    let day_counter = dcg.generate(None)
        .map_err(ManagerError::DayCounterGenerationError)?;

    Ok(Arc::new(CompoundingRateIndex::new(
        p.reference_curve_name,
        p.start_lag,
        p.adjuster,
        tenor,
        calendar,
        day_counter,
        p.daily_past_fixings,
        p.result_compounding,
    )))
}

fn build_index_from_json(
    json_value: serde_json::Value,
    supports: &Supports,
) -> Result<Arc<dyn InterestRateIndex + Send + Sync>, ManagerError> {
    let wrapper: InterestRateIndexJsonProp =
        ManagerError::from_json_or_json_parse_error(json_value)?;

    match wrapper.index_type {
        InterestRateIndexType::TermRate => {
            build_term_rate_index(wrapper.props, supports)
        }
        InterestRateIndexType::CompoundingRate => {
            build_compounding_rate_index(wrapper.props, supports)
        }
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateIndexLoader
// ─────────────────────────────────────────────────────────────────────────────

/// 新版 Loader（取代舊的 InterestRateIndexManager）。
///
/// 負責從 JSON 建立 `InterestRateIndex` 物件並放入 `ManagerBuilder`。
/// 建立完成後呼叫 `builder.build()` 取得不可變的 `FrozenManager`。
///
/// # 使用方式
///
/// ```rust
/// let loader = InterestRateIndexLoader;
/// let mut builder: ManagerBuilder<dyn InterestRateIndex + Send + Sync> =
///     ManagerBuilder::new();
///
/// loader.load_from_reader(&mut builder, "indices.json", &(
///     &frozen_calendar_manager,
///     &frozen_day_counter_manager,
/// ))?;
///
/// let index_manager: FrozenManager<dyn InterestRateIndex + Send + Sync> =
///     builder.build();
/// ```
pub struct InterestRateIndexLoader;

impl<'a> IManager<
    dyn InterestRateIndex + Send + Sync,
    Supports<'a>,
> for InterestRateIndexLoader {
    fn insert_obj_from_json(
        &self,
        builder: &mut ManagerBuilder<dyn InterestRateIndex + Send + Sync>,
        json_value: serde_json::Value,
        supports: &Supports,
    ) -> Result<(), ManagerError> {
        let named: NamedJsonObject =
            ManagerError::from_json_or_json_parse_error(json_value.clone())?;
        let index = build_index_from_json(json_value, supports)?;
        builder.insert(named.name().to_owned(), index);
        Ok(())
    }
}
