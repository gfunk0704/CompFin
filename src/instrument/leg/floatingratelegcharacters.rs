use std::cmp::max;
use std::rc::Rc;

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

// ─────────────────────────────────────────────
// FloatingRateLegCharacters
// ─────────────────────────────────────────────

pub struct FloatingRateLegCharacters {
    generic_characters: GenericLegCharacters,
    leverage: f64,
    spread: f64,
    index: Rc<dyn InterestRateIndex>,
    fixing_rate_calculator: Rc<dyn FixingRateCalculator>,
    taus: Vec<f64>,
}

impl FloatingRateLegCharacters {
    pub fn new(
        generic_characters: GenericLegCharacters,
        leverage: f64,
        spread: f64,
        index: Rc<dyn InterestRateIndex>,
        fixing_rate_calculator: Rc<dyn FixingRateCalculator>,
    ) -> FloatingRateLegCharacters {
        // 改進：用 iterator 取代 for + push，移除不必要的 mut
        let taus: Vec<f64> = generic_characters
            .schedule()
            .schedule_periods()
            .iter()
            .map(|period| {
                let cp = period.calculation_period();
                generic_characters
                    .day_counter()
                    .year_fraction(cp.start_date(), cp.end_date())
            })
            .collect();

        FloatingRateLegCharacters {
            generic_characters,
            leverage,
            spread,
            index,
            fixing_rate_calculator,
            taus,
        }
    }

    pub fn leverage(&self) -> f64 {
        self.leverage
    }

    pub fn spread(&self) -> f64 {
        self.spread
    }

    pub fn index(&self) -> &Rc<dyn InterestRateIndex> {
        self.fixing_rate_calculator.index()
    }

    pub fn fixing_rate_calculator(&self) -> &Rc<dyn FixingRateCalculator> {
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
        let last_period = self
            .generic_characters
            .schedule()
            .schedule_periods()
            .last()
            .unwrap();
        max(
            last_period.payment_date(),
            self.index.end_date(last_period.fixing_date()),
        )
    }

    fn evaluate_flow(
        &self,
        i: usize,
        forward_curve_opt: Option<&Rc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> f64 {
        let fixing_rate = self
            .fixing_rate_calculator
            .fixing(i, forward_curve_opt.unwrap(), pricing_condition);
        let rate = self.leverage * fixing_rate + self.spread;
        self.generic_characters.compounding().future_value(rate, self.taus[i]) - 1.0
    }
}

// ─────────────────────────────────────────────
// FloatingRateLegCharactersGenerator
// ─────────────────────────────────────────────

pub struct FloatingRateLegCharactersGenerator {
    generic_characters_generator: GenericLegCharactersGenerator,
    index: Rc<dyn InterestRateIndex>,
    fixing_rate_calculator_generator: Rc<dyn FixingRateCalculatorGenerator>,
}

impl FloatingRateLegCharactersGenerator {
    pub fn new(
        calendar: Rc<dyn HolidayCalendar>,
        fixing_calendar: Rc<dyn HolidayCalendar>,
        payment_calendar: Rc<dyn HolidayCalendar>,
        schedule_generator: Rc<ScheduleGenerator>,
        day_counter_generator: Rc<DayCounterGenerator>,
        compounding: Compounding,
        setter: LegCharactersSetter,
        index: Rc<dyn InterestRateIndex>,
        fixing_rate_calculator_generator: Rc<dyn FixingRateCalculatorGenerator>,
    ) -> FloatingRateLegCharactersGenerator {
        FloatingRateLegCharactersGenerator {
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

    pub fn index(&self) -> &Rc<dyn InterestRateIndex> {
        &self.index
    }
}

impl LegCharactersGenerator for FloatingRateLegCharactersGenerator {
    fn generic_characters_generator(&self) -> &GenericLegCharactersGenerator {
        &self.generic_characters_generator
    }

    // 改進：實作 generate_with_schedule，消除兩個 generate_with_maturity_* 的重複邏輯
    // generate_with_maturity_date / generate_with_maturity_tenor 的 default 實作會呼叫這裡
    fn generate_with_schedule(&self, schedule: Schedule) -> Rc<dyn LegCharacters> {
        let day_counter = self
            .day_counter_generator()
            .generate(Some(&schedule))
            // 改進：使用 expect 提供明確錯誤訊息，實際上應改為 Result 傳遞
            .expect("DayCounterGenerator failed for FloatingRateLegCharacters");

        let fixing_rate_calculator = self
            .fixing_rate_calculator_generator
            .generate(&schedule);

        let generic_characters = GenericLegCharacters::new(
            self.compounding().clone(),
            day_counter,
            schedule,
        );

        Rc::new(FloatingRateLegCharacters::new(
            generic_characters,
            self.setter().leverage(),
            self.setter().spread(),
            self.index.clone(),
            fixing_rate_calculator,
        ))
    }

    // generate_with_maturity_date 與 generate_with_maturity_tenor
    // 已有 default 實作（在 trait 中），不再需要重複實作
}
