use std::cell::{RefCell, RefMut};
use std::collections::{
    HashMap,
    HashSet
};
use std::rc::Rc;

use chrono::{
    NaiveDate, 
    Weekday
};
use serde::Deserialize;
use serde_json;

use crate::manager::managererror::ManagerError;
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

#[derive(Deserialize)]
struct EasterRelatedHolidayJsonProp {
    easter_type: EasterType,
    shift_days: i32
}

fn easter_related_holiday_from_json(json: serde_json::Value) -> Result<Rc<dyn RecurringHoliday>, ManagerError> {
    let json_prop: EasterRelatedHolidayJsonProp = ManagerError::from_json_or_json_parse_error(json)?;
    let holiday: Rc<dyn RecurringHoliday> = Rc::new(EasterRelatedHoliday::new(json_prop.easter_type, json_prop.shift_days).unwrap());
    Ok(holiday)
}

#[derive(Deserialize)]
struct NthWeekdayHolidayJsonProp {
    month: u32,
    n: u8,
    weekday: Weekday,
}

fn nth_weekday_from_json(json: serde_json::Value) -> Result<Rc<dyn RecurringHoliday>, ManagerError> {
    let json_prop: NthWeekdayHolidayJsonProp = ManagerError::from_json_or_json_parse_error(json)?;
    let holiday: Rc<dyn RecurringHoliday> = Rc::new(NthWeekdayHoliday::new(json_prop.month, json_prop.n, json_prop.weekday).unwrap());
    Ok(holiday)
}


#[derive(Deserialize)]
struct LastWeekdayHolidayJsonProp {
    month: u32,
    weekday: Weekday,
}

fn last_weekday_from_json(json: serde_json::Value) -> Result<Rc<dyn RecurringHoliday>, ManagerError> {
    let json_prop: LastWeekdayHolidayJsonProp = ManagerError::from_json_or_json_parse_error(json)?;
    let holiday: Rc<dyn RecurringHoliday> = Rc::new(LastWeekdayHoliday::new(json_prop.month, json_prop.weekday).unwrap());
    Ok(holiday)
}


#[derive(Deserialize)]
struct FixedDateHolidayJsonProp {
    month: u32,
    day: u32,
    weekend_adjustment_map: HashMap<Weekday, WeekendAdjustment>
}

fn fixed_date_holiday_from_json(json: serde_json::Value) -> Result<Rc<dyn RecurringHoliday>, ManagerError> {
    let json_prop: FixedDateHolidayJsonProp = ManagerError::from_json_or_json_parse_error(json)?;
    let holiday: Rc<dyn RecurringHoliday> = Rc::new(FixedDateHoliday::new(json_prop.month, json_prop.day, &json_prop.weekend_adjustment_map).unwrap());
    Ok(holiday)
}


#[derive(Deserialize)]
enum HolidayType {
    EasterRealted,
    FixedDate,
    NthWeekday,
    LastWeekday
}

#[derive(Deserialize)]
struct HolidayTypedObject {
    holiday_type: HolidayType
}

fn get_recurring_holiday_from_json(json: serde_json::Value) -> Result<Rc<dyn RecurringHoliday>, ManagerError> {
    let holiday_type_obj: HolidayTypedObject = ManagerError::from_json_or_json_parse_error(json.clone())?;
    match holiday_type_obj.holiday_type {
        HolidayType::EasterRealted => easter_related_holiday_from_json(json),
        HolidayType::FixedDate => fixed_date_holiday_from_json(json),
        HolidayType::LastWeekday => last_weekday_from_json(json),
        HolidayType::NthWeekday => nth_weekday_from_json(json)
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


fn get_simple_calendar_from_json(json_value: serde_json::Value) -> Result<Rc<dyn HolidayCalendar>, ManagerError> {
    let holiday_calendar_json: SimpleCalendarJsonProp = ManagerError::from_json_or_json_parse_error(json_value)?;
    let mut recurring_holidays: Vec<Rc<dyn RecurringHoliday>> = Vec::new();
    for recurring_holiday_json in holiday_calendar_json.recurring_holidays.iter() {
        let recurring_holiday = get_recurring_holiday_from_json(recurring_holiday_json.clone())?;
        recurring_holidays.push(recurring_holiday);
    }
    
    let additional_holidays = Vec::from_iter(holiday_calendar_json.additional_holidays.iter().cloned());
    let additional_business_days = Vec::from_iter(holiday_calendar_json.additional_business_days.iter().cloned());
    let simple_calendar = SimpleCalendar::new(
        holiday_calendar_json.weekends, 
        recurring_holidays, 
        additional_holidays, 
        additional_business_days
    );

    if holiday_calendar_json.precomputation.apply {
        let precomputed_simple_calendar = PrecomputedSimpleCalendar::new(
            simple_calendar, 
            holiday_calendar_json.precomputation.start_year, 
            holiday_calendar_json.precomputation.end_year
        );
        Ok(Rc::new(precomputed_simple_calendar))
    } else {
        Ok(Rc::new(simple_calendar))
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

pub struct HolidayCalendarManager {
    map_cell: RefCell<HashMap<String, Rc<dyn HolidayCalendar>>>
}

impl HolidayCalendarManager {
    pub fn new() -> HolidayCalendarManager {
        HolidayCalendarManager {
            map_cell: RefCell::new(HashMap::new())
        }
    }

    pub fn map(&self) -> RefMut<'_, HashMap<String, Rc<dyn HolidayCalendar>>> {
        self.map_cell.borrow_mut()
    }

    pub fn get(&self, name: &String) -> Result<Rc<dyn HolidayCalendar>, ManagerError> {
        let map = self.map();
        let elem_opt = map.get(name);
        elem_opt.map_or(
            Err(ManagerError::NameNotFoundError(name.to_owned())), 
            |elem| Ok(elem.clone())
        )
    }

    pub fn insert_obj_from_json(&self, 
                                json_value: serde_json::Value) -> Result<(), ManagerError> {   
        let named_obj: NamedJsonObject = ManagerError::from_json_or_json_parse_error(json_value.clone())?;                               
        let calendar_typed_object: CalendarTypedObject = ManagerError::from_json_or_json_parse_error(json_value.clone())?;                                        
        match calendar_typed_object.calendar_type {
            CalendarType::SimpleCalendar => {
                let calendar = get_simple_calendar_from_json(json_value)?;
                self.map().insert(named_obj.name().to_owned(), calendar);
                Ok(())
            },
            CalendarType::JointCalendar => {
                let joint_calendar_prop: JointCalendarJsonProp =  ManagerError::from_json_or_json_parse_error(json_value.clone())?; 
                let c1 = self.get(&joint_calendar_prop.c1)?;
                let c2 = self.get(&joint_calendar_prop.c2)?;
                let joint_calendar = match joint_calendar_prop.method_of_joint {
                    MethodOfJoint::Intersection => JointCalendar::intersection(c1, c2),
                    MethodOfJoint::Union => JointCalendar::union(c1, c2)
                };
                self.map().insert(named_obj.name().to_owned(), Rc::new(joint_calendar));
                Ok(())
            }
        }
    }

    pub fn insert_obj_from_json_vec(&self, 
                                    json_vec: &Vec<serde_json::Value>) -> Result<(), ManagerError> {
        let mut remain_indices: Vec<usize> = (0..json_vec.len()).collect();
        let mut result: Result<(), ManagerError> = Ok(());
        loop {
            let mut new_remain_indices: Vec<usize> = Vec::new();
            for index in remain_indices.iter() {
                result = self.insert_obj_from_json(json_vec[*index].clone());
                

                if result.is_err() {
                    new_remain_indices.push(*index);
                }
            }
            if new_remain_indices.len() == 0 ||
                remain_indices == new_remain_indices {
                return result;
            }
            remain_indices = new_remain_indices;
        }
            
    }
}


