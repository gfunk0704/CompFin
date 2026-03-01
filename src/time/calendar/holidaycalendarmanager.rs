use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{NaiveDate, Weekday};
use serde::Deserialize;
use serde_json;

use crate::manager::manager::{IManager, ManagerBuilder};
use crate::manager::managererror::{ManagerError, parse_json_value};
use crate::manager::namedobject::NamedJsonObject;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::calendar::jointcalendar::JointCalendar;
use crate::time::calendar::precomputedsimplecalendar::PrecomputedSimpleCalendar;
use crate::time::calendar::simplecalendar::SimpleCalendar;
use crate::time::recurringholiday::recurringholiday::RecurringHoliday;
use crate::time::recurringholiday::weekendadjustment::WeekendAdjustment;
use crate::time::recurringholiday::fixeddateholiday::FixedDateHoliday;
use crate::time::recurringholiday::nthweekdayholiday::NthWeekdayHoliday;
use crate::time::recurringholiday::lastweekdayholiday::LastWeekdayHoliday;
use crate::time::recurringholiday::easterrelatedholiday::{
    EasterType,
    EasterRelatedHoliday
};

// ─────────────────────────────────────────────────────────────────────────────
// 私有輔助函式（邏輯與原始完全相同，只有 Rc → Arc）
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EasterRelatedHolidayJsonProp {
    easter_type: EasterType,
    shift_days: i32
}

fn easter_related_holiday_from_json(json: serde_json::Value) -> Result<Arc<dyn RecurringHoliday>, ManagerError> {
    let json_prop: EasterRelatedHolidayJsonProp = parse_json_value(json)?;
    let holiday: Arc<dyn RecurringHoliday> = Arc::new(EasterRelatedHoliday::new(json_prop.easter_type, json_prop.shift_days).unwrap());
    Ok(holiday)
}

#[derive(Deserialize)]
struct NthWeekdayHolidayJsonProp {
    month: u32,
    n: u8,
    weekday: Weekday,
}

fn nth_weekday_from_json(json: serde_json::Value) -> Result<Arc<dyn RecurringHoliday>, ManagerError> {
    let json_prop: NthWeekdayHolidayJsonProp = parse_json_value(json)?;
    let holiday: Arc<dyn RecurringHoliday> = Arc::new(NthWeekdayHoliday::new(json_prop.month, json_prop.n, json_prop.weekday).unwrap());
    Ok(holiday)
}

#[derive(Deserialize)]
struct LastWeekdayHolidayJsonProp {
    month: u32,
    weekday: Weekday,
}

fn last_weekday_from_json(json: serde_json::Value) -> Result<Arc<dyn RecurringHoliday>, ManagerError> {
    let json_prop: LastWeekdayHolidayJsonProp = parse_json_value(json)?;
    let holiday: Arc<dyn RecurringHoliday> = Arc::new(LastWeekdayHoliday::new(json_prop.month, json_prop.weekday).unwrap());
    Ok(holiday)
}

#[derive(Deserialize)]
struct FixedDateHolidayJsonProp {
    month: u32,
    day: u32,
    weekend_adjustment_map: HashMap<Weekday, WeekendAdjustment>
}

fn fixed_date_holiday_from_json(json: serde_json::Value) -> Result<Arc<dyn RecurringHoliday>, ManagerError> {
    let json_prop: FixedDateHolidayJsonProp = parse_json_value(json)?;
    let holiday: Arc<dyn RecurringHoliday> = Arc::new(FixedDateHoliday::new(json_prop.month, json_prop.day, &json_prop.weekend_adjustment_map).unwrap());
    Ok(holiday)
}

#[derive(Deserialize)]
enum HolidayType {
    EasterRelated,
    FixedDate,
    NthWeekday,
    LastWeekday
}

#[derive(Deserialize)]
struct HolidayTypedObject {
    holiday_type: HolidayType
}

fn get_recurring_holiday_from_json(json: serde_json::Value) -> Result<Arc<dyn RecurringHoliday>, ManagerError> {
    let holiday_type_obj: HolidayTypedObject = parse_json_value(json.clone())?;
    match holiday_type_obj.holiday_type {
        HolidayType::EasterRelated => easter_related_holiday_from_json(json),
        HolidayType::FixedDate     => fixed_date_holiday_from_json(json),
        HolidayType::LastWeekday   => last_weekday_from_json(json),
        HolidayType::NthWeekday    => nth_weekday_from_json(json)
    }
}

#[derive(Deserialize)]
enum CalendarType {
    SimpleCalendar,
    JointCalendar
}

#[derive(Deserialize)]
struct CalendarTypedObject {
    calendar_type: CalendarType
}

#[derive(Deserialize)]
struct SimpleCalendarPrecomputationJsonProp {
    apply: bool,
    #[serde(default)]
    start_year: i32,
    #[serde(default)]
    end_year: i32
}

#[derive(Deserialize)]
struct SimpleCalendarJsonProp {
    weekends: HashSet<Weekday>,
    recurring_holidays: Vec<serde_json::Value>,
    additional_holidays: Vec<NaiveDate>,
    additional_business_days: Vec<NaiveDate>,
    precomputation: SimpleCalendarPrecomputationJsonProp
}

fn get_simple_calendar_from_json(json_value: serde_json::Value) -> Result<Arc<dyn HolidayCalendar>, ManagerError> {
    let holiday_calendar_json: SimpleCalendarJsonProp = parse_json_value(json_value)?;
    let mut recurring_holidays: Vec<Arc<dyn RecurringHoliday>> = Vec::new();
    for recurring_holiday_json in holiday_calendar_json.recurring_holidays.iter() {
        let recurring_holiday = get_recurring_holiday_from_json(recurring_holiday_json.clone())?;
        recurring_holidays.push(recurring_holiday);
    }

    let simple_calendar = SimpleCalendar::new(
        holiday_calendar_json.weekends,
        recurring_holidays,
        holiday_calendar_json.additional_holidays,
        holiday_calendar_json.additional_business_days
    );

    if holiday_calendar_json.precomputation.apply {
        let precomputed = PrecomputedSimpleCalendar::new(
            simple_calendar,
            holiday_calendar_json.precomputation.start_year,
            holiday_calendar_json.precomputation.end_year
        );
        Ok(Arc::new(precomputed))
    } else {
        Ok(Arc::new(simple_calendar))
    }
}

#[derive(Deserialize)]
enum MethodOfJoint {
    Intersection,
    Union
}

#[derive(Deserialize)]
struct JointCalendarJsonProp {
    c1: String,
    c2: String,
    method_of_joint: MethodOfJoint
}

// ─────────────────────────────────────────────────────────────────────────────
// HolidayCalendarLoader
// ─────────────────────────────────────────────────────────────────────────────

/// 假日曆的載入器，實作 `IManager<dyn HolidayCalendar, ()>`。
///
/// # 相依處理與 retry 機制
///
/// `JointCalendar` 需要兩個已載入的子 calendar（`c1`、`c2`），
/// 而 JSON 陣列中 `JointCalendar` 的順序不一定在子 calendar 之後。
///
/// 解決方式：
/// - `insert_obj_from_json` 在 `JointCalendar` 的情況下呼叫 `builder.get()`。
///   若相依尚未載入，`builder.get()` 回傳 `NotFound`，整個方法回傳 `Err`。
/// - `insert_obj_from_json_vec` 覆寫預設實作，加入 retry loop：
///   每輪把失敗的 index 留到下一輪重試，直到全部成功或連續兩輪沒有進展為止。
///
/// # 與舊 `HolidayCalendarManager` 的對照
///
/// ```text
/// 舊設計                              新設計
/// ─────────────────────────────────   ──────────────────────────────────────
/// HolidayCalendarManager {           HolidayCalendarLoader（無狀態 struct）
///     map_cell: RwLock<HashMap>      └── + ManagerBuilder（外部傳入）
/// }
///
/// self.get(&name)                →   builder.get(&name)
/// self.map().insert(...)         →   builder.insert(...)
/// 自有 insert_obj_from_json_vec  →   覆寫 IManager::insert_obj_from_json_vec
/// ```
///
/// retry loop 的終止條件與演算法邏輯完全不變。
pub struct HolidayCalendarLoader;

impl IManager<dyn HolidayCalendar + Send + Sync, ()> for HolidayCalendarLoader {

    /// 嘗試從 JSON 解析一個 calendar 並插入 builder。
    ///
    /// - `SimpleCalendar`：直接解析，無相依，不會因為載入順序失敗。
    /// - `JointCalendar`：呼叫 `builder.get()` 查詢相依的子 calendar。
    ///   若相依尚未載入，`builder.get()` 回傳 `NotFound`，
    ///   此方法跟著傳播 `Err`，由 retry loop 捕獲並在下一輪重試。
    fn insert_obj_from_json(
        &self,
        builder: &mut ManagerBuilder<dyn HolidayCalendar + Send + Sync>,
        json_value: serde_json::Value,
        _supports: &(),
    ) -> Result<(), ManagerError> {
        let named_obj: NamedJsonObject =
            parse_json_value(json_value.clone())?;
        let calendar_typed_object: CalendarTypedObject =
            parse_json_value(json_value.clone())?;

        match calendar_typed_object.calendar_type {
            CalendarType::SimpleCalendar => {
                let calendar = get_simple_calendar_from_json(json_value)?;
                builder.insert(named_obj.name().to_owned(), calendar);
                Ok(())
            },
            CalendarType::JointCalendar => {
                let joint_prop: JointCalendarJsonProp =
                    parse_json_value(json_value)?;

                // 若 c1 或 c2 尚未載入 → builder.get() 回傳 Err
                // → 此方法回傳 Err → retry loop 下一輪重試
                let c1 = builder.get(&joint_prop.c1)?;
                let c2 = builder.get(&joint_prop.c2)?;

                let joint_calendar = match joint_prop.method_of_joint {
                    MethodOfJoint::Intersection => JointCalendar::intersection(c1, c2),
                    MethodOfJoint::Union        => JointCalendar::union(c1, c2),
                };
                builder.insert(named_obj.name().to_owned(), Arc::new(joint_calendar));
                Ok(())
            }
        }
    }

    /// 覆寫預設實作，加入 retry loop 以處理 JointCalendar 的相依載入順序。
    ///
    /// # 演算法（與原始 `HolidayCalendarManager::insert_obj_from_json_vec` 完全相同）
    /// 1. 初始時所有 index 都在待處理清單 `remain_indices` 中。
    /// 2. 每輪嘗試插入所有待處理 calendar：
    ///    - 成功 → 從清單移除
    ///    - 失敗（相依未就緒）→ 留在清單，下輪重試
    /// 3. 終止條件：
    ///    - `new_remain_indices.is_empty()` → 全部成功，回傳 `Ok(())`
    ///    - `remain_indices == new_remain_indices` → 本輪無任何進展，
    ///      代表有循環相依或格式錯誤，回傳最後一個 `Err`
    fn insert_obj_from_json_vec(
        &self,
        builder: &mut ManagerBuilder<dyn HolidayCalendar + Send + Sync>,
        json_vec: &[serde_json::Value],
        supports: &(),
    ) -> Result<(), ManagerError> {
        let mut remain_indices: Vec<usize> = (0..json_vec.len()).collect();
        let mut result: Result<(), ManagerError> = Ok(());

        loop {
            let mut new_remain_indices: Vec<usize> = Vec::new();

            for &index in remain_indices.iter() {
                result = self.insert_obj_from_json(builder, json_vec[index].clone(), supports);
                if result.is_err() {
                    new_remain_indices.push(index);
                }
            }

            if new_remain_indices.is_empty() || remain_indices == new_remain_indices {
                return result;
            }

            remain_indices = new_remain_indices;
        }
    }
}