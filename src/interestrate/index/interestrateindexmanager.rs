use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

use serde::Deserialize;

use crate::interestrate::compounding::Compounding;
use crate::interestrate::index::interestrateindex::{InterestRateIndex, InterestRateIndexType};
use crate::interestrate::index::termrateindex::TermRateIndex;
use crate::manager::manager::{
    IManager, 
    Manager
};
use crate::manager::managererror::ManagerError;
use crate::manager::namedobject::NamedJsonObject;
use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::calendar::holidaycalendarmanager::HolidayCalendarManager;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::period::{
    ParsePeriodError, 
    Period
};

#[derive(Deserialize)]
struct TermRateIndexJsonProp {
    reference_curve_name: String,
    start_lag: u32,
    adjuster: BusinessDayAdjuster,
    tenor: String,
    calendar: String,
    day_counter_generator: String,
    compounding: Compounding
}


fn get_term_rate_index_from_json(json_value: serde_json::Value,
                                 support: &(&HolidayCalendarManager, &Manager<Rc<DayCounterGenerator>>)) -> Result<Rc<dyn InterestRateIndex>, ManagerError> {
    let json_prop: TermRateIndexJsonProp = ManagerError::from_json_or_json_parse_error(json_value.clone())?;
    let tenor_result: Result<Period, ParsePeriodError> = Period::parse(json_prop.tenor.to_owned());
    if tenor_result.is_err() {
        return Err(ManagerError::TenorParseError(tenor_result.err().unwrap()));
    }
    let tenor: Period = tenor_result.unwrap();
    let calendar = support.0.get(&json_prop.calendar)?;
    let day_counter_generator = support.1.get(&json_prop.day_counter_generator)?;
    let day_counter_result = day_counter_generator.generate(None);
    if day_counter_result.is_err() {
        return Err(ManagerError::DayCounterGenerationError(day_counter_result.err().unwrap()));
    }
    let day_counter = day_counter_result.unwrap();
    Ok(Rc::new(TermRateIndex::new(json_prop.reference_curve_name,
                                  json_prop.start_lag,
                                  json_prop.adjuster,
                                  tenor,
                                  calendar,
                                  day_counter,
                                  json_prop.compounding)))
}


#[derive(Deserialize)]
struct InterestRateIndexJsonProp {
    index_type: InterestRateIndexType,
    props: serde_json::Value
}

fn get_interest_rate_index_from_json(json_value: serde_json::Value,
                                     support: &(&HolidayCalendarManager, &Manager<Rc<DayCounterGenerator>>)) -> Result<Rc<dyn InterestRateIndex>, ManagerError> {
    let json_prop: InterestRateIndexJsonProp = ManagerError::from_json_or_json_parse_error(json_value.clone())?;
    match json_prop.index_type {
        InterestRateIndexType::TermRate => {
            get_term_rate_index_from_json(json_prop.props, support)
        }
    }
}

pub struct InterestRateIndexManager {
    map_cell: RefCell<HashMap<String, Rc<dyn InterestRateIndex>>>
}


impl InterestRateIndexManager {
    pub fn new() -> InterestRateIndexManager {
        InterestRateIndexManager {
            map_cell: RefCell::new(HashMap::new())
        }
    }
}


impl IManager<Rc<dyn InterestRateIndex>, (&HolidayCalendarManager, &Manager<Rc<DayCounterGenerator>>)> for InterestRateIndexManager {
    fn map(&self) -> RefMut<'_, HashMap<String, Rc<dyn InterestRateIndex>>> {
        self.map_cell.borrow_mut()
    }

    fn insert_obj_from_json(&self, 
                            json_value: serde_json::Value,
                            supports: &(&HolidayCalendarManager, &Manager<Rc<DayCounterGenerator>>)) -> Result<(), ManagerError> {
        let named_object: NamedJsonObject = ManagerError::from_json_or_json_parse_error(json_value.clone())?;
        let index_obj = get_interest_rate_index_from_json(json_value, supports)?;
        let mut map = self.map();
        map.insert(named_object.name().to_owned(), index_obj);
        Ok(())
    }
}