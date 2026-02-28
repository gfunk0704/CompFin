// ── interestrateindexmanager.rs ──────────────────────────────────────────────

use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::interestrate::compounding::Compounding;
use crate::interestrate::index::compoundingconvention::{FixingConvention, MissingFixingHandler};
use crate::interestrate::index::compoundingrateindex::CompoundingRateIndex;
use crate::interestrate::index::interestrateindex::{InterestRateIndex, InterestRateIndexType};
use crate::interestrate::index::termrateindex::TermRateIndex;
use crate::manager::manager::{IManager, ManagerBuilder, FrozenManager};
use crate::manager::managererror::ManagerError;
use crate::manager::namedobject::NamedJsonObject;
use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::period::Period;


type Supports<'a> = (
    &'a FrozenManager<dyn HolidayCalendar + Send + Sync>,
    &'a FrozenManager<DayCounterGenerator>,
);

fn parse_period(s: String) -> Result<Period, ManagerError> {
    Period::parse(&s).map_err(ManagerError::TenorParseError)
}


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
    #[serde(default)]
    past_fixings: HashMap<NaiveDate, f64>,
}

/// JSON prop for CompoundingRateIndex.
///
/// # 欄位說明
///
/// - `lookback_days`：observation lag（業務日數）。SOFR standard = 0，lookback = 2。
///   若省略，預設 0。
/// - `fixing_convention`：`"Advance"` 或 `"Arrear"`。若省略，預設 `"Advance"`。
/// - `missing_fixing_handler`：`"Null"` 或 `"PreviousFixing"`。若省略，預設 `"Null"`。
/// - `fixing_calendar`：fixing date 使用的 calendar 名稱。
///   若省略，與 `calendar` 相同（適用 lookback_days == 0 的情況）。
#[derive(Deserialize)]
struct CompoundingRateIndexJsonProp {
    reference_curve_name: String,
    start_lag: u32,
    adjuster: BusinessDayAdjuster,
    tenor: String,
    calendar: String,
    #[serde(default)]
    fixing_calendar: Option<String>,
    day_counter_generator: String,
    result_compounding: Compounding,
    #[serde(default)]
    daily_past_fixings: HashMap<NaiveDate, f64>,
    #[serde(default)]
    lookback_days: u32,
    #[serde(default)]
    lockout_days: u32,
    #[serde(default = "default_fixing_convention")]
    fixing_convention: String,
    #[serde(default = "default_missing_fixing_handler")]
    missing_fixing_handler: String,
}

fn default_fixing_convention() -> String { "Advance".to_string() }
fn default_missing_fixing_handler() -> String { "Null".to_string() }

fn parse_fixing_convention(s: &str) -> Result<FixingConvention, ManagerError> {
    match s {
        "Advance" => Ok(FixingConvention::Advance),
        "Arrear"  => Ok(FixingConvention::Arrear),
        other     => Err(ManagerError::JsonParseError(
            serde_json::from_str::<serde_json::Value>(
                &format!("\"Unknown fixing_convention: {other}\"")
            ).unwrap_err()
        )),
    }
}

fn parse_missing_fixing_handler(s: &str) -> Result<MissingFixingHandler, ManagerError> {
    match s {
        "Null"           => Ok(MissingFixingHandler::Null),
        "PreviousFixing" => Ok(MissingFixingHandler::PreviousFixing),
        other            => Err(ManagerError::JsonParseError(
            serde_json::from_str::<serde_json::Value>(
                &format!("\"Unknown missing_fixing_handler: {other}\"")
            ).unwrap_err()
        )),
    }
}

#[derive(Deserialize)]
struct InterestRateIndexJsonProp {
    index_type: InterestRateIndexType,
    props: serde_json::Value,
}


// ─────────────────────────────────────────────────────────────────────────────
// 工廠函式
// ─────────────────────────────────────────────────────────────────────────────

fn build_term_rate_index(
    json_value: serde_json::Value,
    supports: &Supports,
) -> Result<Arc<dyn InterestRateIndex + Send + Sync>, ManagerError> {
    let p: TermRateIndexJsonProp =
        ManagerError::from_json_or_json_parse_error(json_value)?;

    let tenor      = parse_period(p.tenor)?;
    let calendar   = supports.0.get(&p.calendar)?;
    let dcg        = supports.1.get(&p.day_counter_generator)?;
    let day_counter = dcg.generate(None).map_err(ManagerError::DayCounterGenerationError)?;

    Ok(Arc::new(TermRateIndex::new(
        p.reference_curve_name, p.start_lag, p.adjuster, tenor,
        calendar, day_counter, p.compounding, p.past_fixings,
    )))
}

fn build_compounding_rate_index(
    json_value: serde_json::Value,
    supports: &Supports,
) -> Result<Arc<dyn InterestRateIndex + Send + Sync>, ManagerError> {
    let p: CompoundingRateIndexJsonProp =
        ManagerError::from_json_or_json_parse_error(json_value)?;

    let tenor           = parse_period(p.tenor)?;
    let calendar        = supports.0.get(&p.calendar)?;
    let fixing_calendar = match &p.fixing_calendar {
        Some(name) => supports.0.get(name)?,
        None       => calendar.clone(),
    };
    let dcg         = supports.1.get(&p.day_counter_generator)?;
    let day_counter = dcg.generate(None).map_err(ManagerError::DayCounterGenerationError)?;
    let fixing_conv = parse_fixing_convention(&p.fixing_convention)?;
    let missing_fix = parse_missing_fixing_handler(&p.missing_fixing_handler)?;

    Ok(Arc::new(CompoundingRateIndex::with_options(
        p.reference_curve_name, p.start_lag, p.adjuster, tenor,
        calendar, fixing_calendar, day_counter, p.daily_past_fixings, p.result_compounding,
        p.lookback_days, p.lockout_days, fixing_conv, missing_fix,
    )))
}

fn build_index_from_json(
    json_value: serde_json::Value,
    supports: &Supports,
) -> Result<Arc<dyn InterestRateIndex + Send + Sync>, ManagerError> {
    let wrapper: InterestRateIndexJsonProp =
        ManagerError::from_json_or_json_parse_error(json_value)?;

    match wrapper.index_type {
        InterestRateIndexType::TermRate       => build_term_rate_index(wrapper.props, supports),
        InterestRateIndexType::CompoundingRate => build_compounding_rate_index(wrapper.props, supports),
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateIndexLoader
// ─────────────────────────────────────────────────────────────────────────────

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
