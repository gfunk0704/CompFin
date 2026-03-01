use std::sync::Arc; // 變更：Rc → Arc

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
use crate::manager::manager::SimpleLoader; // 變更：Manager → SimpleLoader
use crate::manager::managererror::{ManagerError, parse_json_value};

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

/// # 變更說明
/// 回傳型別由 `Rc<DayCounterGenerator>` 改為 `Arc<DayCounterGenerator>`。
/// 內部 numerator/dominator generator 的 `Rc` 也改為 `Arc`，
/// 使 `DayCounterGenerator` 本身成為 `Send + Sync`，可放入 `FrozenManager`。
fn get_day_counter_generator_from_json(
    json_value: serde_json::Value,
) -> Result<Arc<DayCounterGenerator>, ManagerError> { // 變更：Rc → Arc
    let json_prop: DayCounterGeneratorJsonProp =
        parse_json_value(json_value)?;

    let numerator_typed_object: DayCounterNumeratorTypedObject =
        parse_json_value(json_prop.numerator.clone())?;

    let numerator_generator: Arc<dyn DayCounterNumeratorGenerator> = // 變更：Rc → Arc
        match numerator_typed_object.numerator_type {
            DayCounterNumeratorType::Actual => Arc::new(ActualNumeratorGenerator::new()),
            DayCounterNumeratorType::NoLeap => Arc::new(NoLeapNumeratorGenerator::new()),
            DayCounterNumeratorType::One    => Arc::new(OneNumeratorGenerator::new()),
            DayCounterNumeratorType::Thirty => {
                let generator: ThirtyNumeratorGenerator =
                    parse_json_value(json_prop.numerator)?;
                Arc::new(generator)
            }
        };

    let dominator_typed_object: DayCounterDominatorTypedObject =
        parse_json_value(json_prop.dominator.clone())?;

    let dominator_generator: Arc<dyn DayCounterDominatorGenerator> = // 變更：Rc → Arc
        match dominator_typed_object.dominator_type {
            DayCounterDominatorType::Const => {
                let generator: ConstDayCounterDominatorGenerator =
                    parse_json_value(json_prop.dominator)?;
                Arc::new(generator)
            },
            DayCounterDominatorType::ICMAActual  => Arc::new(ICMADayCounterDominatorGenerator::new()),
            DayCounterDominatorType::ISDAActual  => Arc::new(ISDAActualDayCounterDominatorGenerator::new()),
        };

    Ok(Arc::new(DayCounterGenerator::new(
        numerator_generator,
        dominator_generator,
        json_prop.include_d1,
        json_prop.include_d2,
    )))
}

pub struct DayCounterGeneratorManager;

impl DayCounterGeneratorManager {
    /// # 變更說明
    /// 回傳 `SimpleLoader<DayCounterGenerator>` 取代原本的 `Manager<Rc<DayCounterGenerator>>`。
    ///
    /// 使用方式：
    /// ```rust
    /// let mut builder: ManagerBuilder<DayCounterGenerator> = ManagerBuilder::new();
    /// DayCounterGeneratorManager::new_loader()
    ///     .insert_obj_from_json_vec(&mut builder, &json_vec, &())?;
    /// let frozen: FrozenManager<DayCounterGenerator> = builder.build();
    /// ```
    pub fn new_loader() -> SimpleLoader<DayCounterGenerator> { // 變更：Manager → SimpleLoader
        SimpleLoader::new(get_day_counter_generator_from_json)
    }
}
