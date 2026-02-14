use std::rc::Rc;

use serde::Deserialize;

use crate::time::daycounter::constdaycounterdominator::ConstDayCounterDominatorGenerator;
use crate::time::daycounter::daycounter::{
    DayCounterNumeratorGenerator,
    DayCounterDominatorGenerator,
    DayCounterGenerator
};
use crate::time::daycounter::icmaactualdaycountdominator::ICMADayCounterDominatorGenerator;
use crate::time::daycounter::isdaactualdaycounterdominator::ISDAActualDayCounterDominatorGenerator;
use crate::time::daycounter::numerator::actualnumerator::ActualNumeratorGenerator;
use crate::time::daycounter::numerator::noleapnumerator::NoLeapNumeratorGenerator;
use crate::time::daycounter::numerator::onenumerator::OneNumeratorGenerator;
use crate::time::daycounter::numerator::thirtynumerator::ThirtyNumeratorGenerator;
use crate::manager::manager::Manager;
use crate::manager::managererror::ManagerError;

#[derive(Deserialize)]
pub enum DayCounterNumeratorType {
    Actual,
    NoLeap,
    One,
    Thirty
}


#[derive(Deserialize)]
struct DayCounterNumeratorTypedObject {
    numerator_type: DayCounterNumeratorType
}


#[derive(Deserialize)]
pub enum DayCounterDominatorType {
    Const,
    ICMAActual,
    ISDAActual
}


#[derive(Deserialize)]
struct DayCounterDominatorTypedObject {
    dominator_type: DayCounterDominatorType
}


#[derive(Deserialize)]
struct DayCounterGeneratorJsonProp {
    numerator: serde_json::Value,
    dominator: serde_json::Value,
    include_d1: bool,
    include_d2: bool
}



fn get_day_counter_generator_from_json(json_value: serde_json::Value) -> Result<Rc<DayCounterGenerator>, ManagerError> {
    let json_prop: DayCounterGeneratorJsonProp = ManagerError::from_json_or_json_parse_error(json_value.clone())?;
    let numerator_typed_object: DayCounterNumeratorTypedObject = ManagerError::from_json_or_json_parse_error(json_prop.numerator.clone())?;
    let numerator_generator: Rc<dyn DayCounterNumeratorGenerator> = match numerator_typed_object.numerator_type {
        DayCounterNumeratorType::Actual => {
            Rc::new(ActualNumeratorGenerator::new())
        },
        DayCounterNumeratorType::NoLeap => {
            Rc::new(NoLeapNumeratorGenerator::new())
        },
        DayCounterNumeratorType::One => {
            Rc::new(OneNumeratorGenerator::new())
        },
        DayCounterNumeratorType::Thirty => {
            let thirty_numerator_generator: ThirtyNumeratorGenerator = ManagerError::from_json_or_json_parse_error(json_prop.numerator)?;
            Rc::new(thirty_numerator_generator)
        }
    };
    let dominator_typed_object: DayCounterDominatorTypedObject = ManagerError::from_json_or_json_parse_error(json_prop.dominator.clone())?;
    let dominator_generator: Rc<dyn DayCounterDominatorGenerator> = match  dominator_typed_object.dominator_type {
        DayCounterDominatorType::Const => {
            let const_dominator_generator: ConstDayCounterDominatorGenerator = ManagerError::from_json_or_json_parse_error(json_prop.dominator)?;
            Rc::new(const_dominator_generator)
        },
        DayCounterDominatorType::ICMAActual => {
            Rc::new(ICMADayCounterDominatorGenerator::new())
        }
        DayCounterDominatorType::ISDAActual => {
            Rc::new(ISDAActualDayCounterDominatorGenerator::new())
        }
    };
    Ok(Rc::new(DayCounterGenerator::new(numerator_generator, dominator_generator, json_prop.include_d1, json_prop.include_d2)))
}


pub struct DayCounterGeneratorManager;


impl DayCounterGeneratorManager {
    pub fn new() -> Manager<Rc<DayCounterGenerator>> {
        Manager::new(get_day_counter_generator_from_json)
    }
}





