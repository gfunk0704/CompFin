use std::cell::{
    RefCell, 
    RefMut
};
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;

use serde::Deserialize;


use crate::manager::managererror::ManagerError;
use crate::manager::manager::{
    IManager, 
    Manager
};
use crate::time::calendar::holidaycalendarmanager::HolidayCalendarManager;
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

pub struct Configuration {
    holiyday_calendar_manager_cell: RefCell<HolidayCalendarManager>,
    schedule_generator_manager_cell: RefCell<Manager<ScheduleGenerator>>,
    day_counter_generator_manager_cell: RefCell<Manager<Rc<DayCounterGenerator>>>
}


impl Configuration {
    pub fn new() -> Configuration {
        let holiday_calendar_manager = HolidayCalendarManager::new();
        let schedule_generator_manager = ScheduleGeneratorManager::new();
        let day_counter_generator_manager = DayCounterGeneratorManager::new();
        Configuration {
            holiyday_calendar_manager_cell: RefCell::new(holiday_calendar_manager),
            schedule_generator_manager_cell: RefCell::new(schedule_generator_manager),
            day_counter_generator_manager_cell: RefCell::new(day_counter_generator_manager)
        }
    }

    pub fn holiyday_calendar_manager(&self) -> RefMut<'_, HolidayCalendarManager> {
        let borrow = self.holiyday_calendar_manager_cell.borrow_mut();
        borrow
    }

    pub fn schedule_generator_manager(&self) -> RefMut<'_, Manager<ScheduleGenerator>> {
        let borrow = self.schedule_generator_manager_cell.borrow_mut();
        borrow
    }

    pub fn day_counter_generator_manager(&self) -> RefMut<'_, Manager<Rc<DayCounterGenerator>>> {
        let borrow = self.day_counter_generator_manager_cell.borrow_mut();
        borrow
    }

    pub fn from_reader(&self, file_path: String) -> Result<(), ManagerError> {
        let file = File::open(file_path).map_err(|error| ManagerError::IOError(error))?;
        let reader = BufReader::new(file);
        let json_prop: ConfigurationJsonProp = serde_json::from_reader(reader).map_err(|error| ManagerError::JsonParseError(error))?;
        let _empty_support = ();
        let holiyday_calendar_manager = self.holiyday_calendar_manager_cell.borrow_mut();
        let _ = holiyday_calendar_manager.insert_obj_from_json_vec(&json_prop.holiday_calendar)?;
        let schedule_generator_manager= self.schedule_generator_manager_cell.borrow_mut();
        let _ = schedule_generator_manager.insert_obj_from_json_vec(&json_prop.schedule, &_empty_support)?;
        let day_counter_generator_manager = self.day_counter_generator_manager_cell.borrow_mut();
        let _ = day_counter_generator_manager.insert_obj_from_json_vec(&json_prop.day_count, &_empty_support)?;
        Ok(())
    }
}

