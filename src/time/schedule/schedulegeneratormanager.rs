use serde::Deserialize;

use crate::manager::manager::Manager;
use crate::manager::managererror::ManagerError;
use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::period::Period;
use crate::time::schedule::calculationperiodgenerator::{
    CalculationPeriodGenerator,
    GenerationMode
};
use crate::time::schedule::generationdirection::GenerationDirection;
use crate::time::schedule::schedule::ScheduleGenerator;
use crate::time::schedule::stubadjuster::StubConvention;

#[derive(Deserialize)]
struct CalculationPeriodGeneratorJsonProp {
    start_lag: i32,
    frequency: String,
    freq_adjuster: BusinessDayAdjuster,
    mat_adjuster: BusinessDayAdjuster,
    mode: GenerationMode,
    direction: GenerationDirection,
    stub_convention: StubConvention
}


fn get_calculation_period_generator_from_json(json_value: serde_json::Value) -> Result<CalculationPeriodGenerator, ManagerError> {        
    let generator_props: CalculationPeriodGeneratorJsonProp = ManagerError::from_json_or_json_parse_error(json_value)?;
    let frequency_result = Period::parse(generator_props.frequency);
        
    if frequency_result.is_err() {
        return Err(ManagerError::TenorParseError(frequency_result.err().unwrap()));
    }

    let generator = CalculationPeriodGenerator::new(
        generator_props.start_lag, 
        frequency_result.unwrap(), 
        generator_props.freq_adjuster, 
        generator_props.mat_adjuster, 
        generator_props.mode, 
        generator_props.direction, 
        generator_props.stub_convention
    );

    Ok(generator)
}


#[derive(Deserialize)]
enum RelativeDateGeneratorType {
    ShiftDays,
    FeequencyRatio
}


#[derive(Deserialize)]
struct RelativeDateGeneratorTypedObject {
    generator_type: RelativeDateGeneratorType
}




#[derive(Deserialize)]
struct ScheduleGeneratorJsonProp {
    calculation_period_generator: serde_json::Value,
    fixing_date_generator: serde_json::Value,
    payment_date_generator: serde_json::Value
}


fn get_schedule_generator_from_json(json_value: serde_json::Value) -> Result<ScheduleGenerator, ManagerError> {
    let generator_prop: ScheduleGeneratorJsonProp = ManagerError::from_json_or_json_parse_error(json_value)?;
    let calculation_period_generator = get_calculation_period_generator_from_json(generator_prop.calculation_period_generator)?;
    let fixing_date_generator = ManagerError::from_json_or_json_parse_error(generator_prop.fixing_date_generator)?;
    let payment_date_generator = ManagerError::from_json_or_json_parse_error(generator_prop.payment_date_generator)?;
    let generator = ScheduleGenerator::new(
        calculation_period_generator, 
        fixing_date_generator, 
        payment_date_generator
    );
    Ok(generator)
}


pub struct ScheduleGeneratorManager;


impl ScheduleGeneratorManager {
    pub fn new() -> Manager<ScheduleGenerator> {
        Manager::new(get_schedule_generator_from_json)
    }
}