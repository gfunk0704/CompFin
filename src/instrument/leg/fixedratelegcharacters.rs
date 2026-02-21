use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::leg::legcharacters::{
    GenericLegCharacters,
    GenericLegCharactersGenerator,
    LegCharacters,
    LegCharactersGenerator,
    LegCharactersSetter,
};
use crate::interestrate::compounding::Compounding;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::schedule::schedule::{Schedule, ScheduleGenerator};

// ─────────────────────────────────────────────
// FixedRateLegCharacters
// ─────────────────────────────────────────────

pub struct FixedRateLegCharacters {
    generic_characters: GenericLegCharacters,
    fixed_rate: f64,
    flow_values: Vec<f64>,
}

impl FixedRateLegCharacters {
    pub fn new(
        generic_characters: GenericLegCharacters,
        fixed_rate: f64,
    ) -> FixedRateLegCharacters {
        // 改進：用 iterator 取代 for + push，移除不必要的 mut
        let flow_values: Vec<f64> = generic_characters
            .schedule()
            .schedule_periods()
            .iter()
            .map(|period| {
                let cp = period.calculation_period();
                let tau = generic_characters
                    .day_counter()
                    .year_fraction(cp.start_date(), cp.end_date());
                generic_characters.compounding().future_value(fixed_rate, tau) - 1.0
            })
            .collect();

        FixedRateLegCharacters {
            generic_characters,
            fixed_rate,
            flow_values,
        }
    }

    pub fn fixed_rate(&self) -> f64 {
        self.fixed_rate
    }
}

impl LegCharacters for FixedRateLegCharacters {
    fn reference_curve_name(&self) -> Option<&String> {
        None
    }

    fn generic_characters(&self) -> &GenericLegCharacters {
        &self.generic_characters
    }

    fn max_date(&self) -> NaiveDate {
        self.generic_characters.maturity_date()
    }

    fn evaluate_flow(
        &self,
        i: usize,
        _forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        _pricing_condition: &PricingCondition,
    ) -> f64 {
        self.flow_values[i]
    }
}

// ─────────────────────────────────────────────
// FixedRateLegCharactersGenerator
// ─────────────────────────────────────────────

pub struct FixedRateLegCharactersGenerator {
    generic_characters_generator: GenericLegCharactersGenerator,
}

impl FixedRateLegCharactersGenerator {
    pub fn new(
        calendar: Arc<dyn HolidayCalendar>,
        fixing_calendar: Arc<dyn HolidayCalendar>,
        payment_calendar: Arc<dyn HolidayCalendar>,
        schedule_generator: Arc<ScheduleGenerator>,
        day_counter_generator: Arc<DayCounterGenerator>,
        compounding: Compounding,
        setter: LegCharactersSetter,
    ) -> FixedRateLegCharactersGenerator {
        FixedRateLegCharactersGenerator {
            generic_characters_generator: GenericLegCharactersGenerator::new(
                calendar,
                fixing_calendar,
                payment_calendar,
                schedule_generator,
                day_counter_generator,
                compounding,
                setter,
            ),
        }
    }
}

impl LegCharactersGenerator for FixedRateLegCharactersGenerator {
    fn generic_characters_generator(&self) -> &GenericLegCharactersGenerator {
        &self.generic_characters_generator
    }

    // 改進：實作 generate_with_schedule，消除兩個 generate_with_maturity_* 的重複邏輯
    // generate_with_maturity_date / generate_with_maturity_tenor 的 default 實作會呼叫這裡
    fn generate_with_schedule(&self, schedule: Schedule) -> Arc<dyn LegCharacters> {
        let day_counter = self
            .day_counter_generator()
            .generate(Some(&schedule))
            // 改進：使用 expect 提供明確錯誤訊息，實際上應改為 Result 傳遞
            .expect("DayCounterGenerator failed for FixedRateLegCharacters");

        let generic_characters = GenericLegCharacters::new(
            self.compounding().clone(),
            day_counter,
            schedule,
        );

        Arc::new(FixedRateLegCharacters::new(
            generic_characters,
            self.setter().fixed_rate(),
        ))
    }

    // generate_with_maturity_date 與 generate_with_maturity_tenor
    // 已有 default 實作（在 trait 中），不再需要重複實作
    // 若需要覆寫可在此加上，但預設行為已足夠
}
