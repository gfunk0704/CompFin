use std::cmp::max;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::instrument::leg::fixingratecalculator::fixingratecalculator::{
    FixingRateCalculator,
    FixingRateCalculatorGenerator,
};
use crate::instrument::leg::legcharacters::{
    GenericLegCharacters,
    GenericLegCharactersGenerator,
    LegCharacters,
    LegCharactersGenerator,
    LegCharactersSetter,
};
use crate::interestrate::compounding::Compounding;
use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::schedule::schedule::{Schedule, ScheduleGenerator};


// ─────────────────────────────────────────────────────────────────────────────
// FloatingRateLegCharacters
// ─────────────────────────────────────────────────────────────────────────────

pub struct FloatingRateLegCharacters {
    generic_characters: GenericLegCharacters,
    leverage: f64,
    spread: f64,
    index: Arc<dyn InterestRateIndex + Send + Sync>,
    fixing_rate_calculator: Arc<dyn FixingRateCalculator>,
    taus: Vec<f64>,
}

impl FloatingRateLegCharacters {
    pub fn new(
        generic_characters: GenericLegCharacters,
        leverage: f64,
        spread: f64,
        index: Arc<dyn InterestRateIndex + Send + Sync>,
        fixing_rate_calculator: Arc<dyn FixingRateCalculator>,
    ) -> Self {
        let taus = generic_characters
            .schedule()
            .schedule_periods()
            .iter()
            .map(|sp| {
                let cp = sp.calculation_period();
                generic_characters
                    .day_counter()
                    .year_fraction(cp.start_date(), cp.end_date())
            })
            .collect();

        Self {
            generic_characters,
            leverage,
            spread,
            index,
            fixing_rate_calculator,
            taus,
        }
    }

    pub fn leverage(&self) -> f64 { self.leverage }
    pub fn spread(&self) -> f64 { self.spread }

    pub fn fixing_rate_calculator(&self) -> &Arc<dyn FixingRateCalculator> {
        &self.fixing_rate_calculator
    }
}

impl LegCharacters for FloatingRateLegCharacters {
    fn reference_curve_name(&self) -> Option<&String> {
        Some(self.index.reference_curve_name())
    }

    fn generic_characters(&self) -> &GenericLegCharacters {
        &self.generic_characters
    }

    fn max_date(&self) -> NaiveDate {
        let last = self
            .generic_characters
            .schedule()
            .schedule_periods()
            .last()
            .unwrap();
        max(
            last.payment_date(),
            self.index.end_date(last.fixing_date()),
        )
    }

    fn evaluate_flow(
        &self,
        i: usize,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> f64 {
        let fixing_rate = self
            .fixing_rate_calculator
            .fixing(i, forward_curve_opt.unwrap(), pricing_condition);
        let rate = self.leverage * fixing_rate + self.spread;
        self.generic_characters.compounding().future_value(rate, self.taus[i]) - 1.0
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// FloatingRateLegCharactersGenerator
// ─────────────────────────────────────────────────────────────────────────────

pub struct FloatingRateLegCharactersGenerator {
    generic_characters_generator: GenericLegCharactersGenerator,
    index: Arc<dyn InterestRateIndex + Send + Sync>,
    fixing_rate_calculator_generator: Arc<dyn FixingRateCalculatorGenerator>,
}

impl FloatingRateLegCharactersGenerator {
    pub fn new(
        calendar: Arc<dyn HolidayCalendar>,
        fixing_calendar: Arc<dyn HolidayCalendar>,
        payment_calendar: Arc<dyn HolidayCalendar>,
        schedule_generator: Arc<ScheduleGenerator>,
        day_counter_generator: Arc<DayCounterGenerator>,
        compounding: Compounding,
        setter: LegCharactersSetter,
        index: Arc<dyn InterestRateIndex + Send + Sync>,
        fixing_rate_calculator_generator: Arc<dyn FixingRateCalculatorGenerator>,
    ) -> Self {
        Self {
            generic_characters_generator: GenericLegCharactersGenerator::new(
                calendar,
                fixing_calendar,
                payment_calendar,
                schedule_generator,
                day_counter_generator,
                compounding,
                setter,
            ),
            index,
            fixing_rate_calculator_generator,
        }
    }
}

impl LegCharactersGenerator for FloatingRateLegCharactersGenerator {
    fn generic_characters_generator(&self) -> &GenericLegCharactersGenerator {
        &self.generic_characters_generator
    }

    fn generate_with_schedule(&self, schedule: Schedule) -> Arc<dyn LegCharacters> {
        let day_counter = self
            .day_counter_generator()
            .generate(Some(&schedule))
            .expect("DayCounterGenerator failed for FloatingRateLegCharacters");

        let fixing_rate_calculator = self
            .fixing_rate_calculator_generator
            .generate(&schedule);

        let generic_characters = GenericLegCharacters::new(
            *self.compounding(),
            day_counter,
            schedule,
        );

        Arc::new(FloatingRateLegCharacters::new(
            generic_characters,
            self.setter().leverage(),
            self.setter().spread(),
            self.index.clone(),
            fixing_rate_calculator,
        ))
    }
}
