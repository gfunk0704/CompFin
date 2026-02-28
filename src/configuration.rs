use std::fs::File;
use std::io::BufReader;

use serde::Deserialize;

use crate::manager::manager::{
    FrozenManager,
    IManager,
    ManagerBuilder,
};
use crate::manager::managererror::ManagerError;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::calendar::holidaycalendarmanager::HolidayCalendarLoader;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::daycounter::daycountergeneratormanager::DayCounterGeneratorManager;
use crate::time::schedule::schedule::ScheduleGenerator;
use crate::time::schedule::schedulegeneratormanager::ScheduleGeneratorManager;


#[derive(Deserialize)]
struct ConfigurationJsonProp {
    holiday_calendar: Vec<serde_json::Value>,
    schedule: Vec<serde_json::Value>,
    day_count: Vec<serde_json::Value>
}

/// 系統設定容器。
///
/// 在 `from_reader` 中完成所有載入與凍結，
/// 之後所有 manager 均為不可變的 `FrozenManager`，可安全跨執行緒共享。
pub struct Configuration {
    holiday_calendar_manager: FrozenManager<dyn HolidayCalendar + Send + Sync>,
    schedule_generator_manager: FrozenManager<ScheduleGenerator>,
    day_counter_generator_manager: FrozenManager<DayCounterGenerator>,
}

impl Configuration {
    /// 從 JSON 設定檔建立 `Configuration`。
    ///
    /// 載入順序：holiday calendar → schedule generator → day counter generator。
    pub fn from_reader(file_path: &str) -> Result<Configuration, ManagerError> {
        let file = File::open(file_path).map_err(ManagerError::IOError)?;
        let reader = BufReader::new(file);
        let json_prop: ConfigurationJsonProp = serde_json::from_reader(reader)
            .map_err(ManagerError::JsonParseError)?;

        // ── Holiday Calendar ──────────────────────────────────────────────────
        let mut cal_builder: ManagerBuilder<dyn HolidayCalendar + Send + Sync> =
            ManagerBuilder::new();
        let cal_loader = HolidayCalendarLoader;
        cal_loader.insert_obj_from_json_vec(&mut cal_builder, &json_prop.holiday_calendar, &())?;
        let holiday_calendar_manager = cal_builder.build();

        // ── Schedule Generator ────────────────────────────────────────────────
        let mut sched_builder: ManagerBuilder<ScheduleGenerator> = ManagerBuilder::new();
        ScheduleGeneratorManager::new_loader()
            .insert_obj_from_json_vec(&mut sched_builder, &json_prop.schedule, &())?;
        let schedule_generator_manager = sched_builder.build();

        // ── Day Counter Generator ─────────────────────────────────────────────
        let mut dc_builder: ManagerBuilder<DayCounterGenerator> = ManagerBuilder::new();
        DayCounterGeneratorManager::new_loader()
            .insert_obj_from_json_vec(&mut dc_builder, &json_prop.day_count, &())?;
        let day_counter_generator_manager = dc_builder.build();

        Ok(Configuration {
            holiday_calendar_manager,
            schedule_generator_manager,
            day_counter_generator_manager,
        })
    }

    pub fn holiday_calendar_manager(&self) -> &FrozenManager<dyn HolidayCalendar + Send + Sync> {
        &self.holiday_calendar_manager
    }

    pub fn schedule_generator_manager(&self) -> &FrozenManager<ScheduleGenerator> {
        &self.schedule_generator_manager
    }

    pub fn day_counter_generator_manager(&self) -> &FrozenManager<DayCounterGenerator> {
        &self.day_counter_generator_manager
    }
}
