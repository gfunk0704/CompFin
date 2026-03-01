use std::sync::Arc;

use serde::Deserialize;

use crate::manager::manager::{SimpleLoader};
use crate::manager::managererror::{ManagerError, parse_json_value};
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
    let generator_props: CalculationPeriodGeneratorJsonProp = parse_json_value(json_value)?;
    let frequency_result = Period::parse(&generator_props.frequency);
        
    if frequency_result.is_err() {
        return Err(frequency_result.err().unwrap().into());
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
struct ScheduleGeneratorJsonProp {
    calculation_period_generator: serde_json::Value,
    fixing_date_generator: serde_json::Value,
    payment_date_generator: serde_json::Value
}


fn get_schedule_generator_from_json(json_value: serde_json::Value) -> Result<Arc<ScheduleGenerator>, ManagerError> {
    let generator_prop: ScheduleGeneratorJsonProp = parse_json_value(json_value)?;
    let calculation_period_generator = get_calculation_period_generator_from_json(generator_prop.calculation_period_generator)?;
    let fixing_date_generator = parse_json_value(generator_prop.fixing_date_generator)?;
    let payment_date_generator = parse_json_value(generator_prop.payment_date_generator)?;
    let generator = ScheduleGenerator::new(
        calculation_period_generator, 
        fixing_date_generator, 
        payment_date_generator
    );
    Ok(Arc::new(generator))
}


pub struct ScheduleGeneratorManager;


impl ScheduleGeneratorManager {
    /// 回傳 `SimpleLoader<ScheduleGenerator>` 取代原本的 `Manager<ScheduleGenerator>`。
    ///
    /// 使用方式：
    /// ```rust
    /// let mut builder: ManagerBuilder<ScheduleGenerator> = ManagerBuilder::new();
    /// ScheduleGeneratorManager::new_loader()
    ///     .insert_obj_from_json_vec(&mut builder, &json_vec, &())?;
    /// let frozen: FrozenManager<ScheduleGenerator> = builder.build();
    /// ```
    pub fn new_loader() -> SimpleLoader<ScheduleGenerator> {
        SimpleLoader::new(get_schedule_generator_from_json)
    }
}