use std::sync::Arc;

use chrono::NaiveDate;

use crate::interestrate::compounding::Compounding;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::{DayCounter, DayCounterGenerator};
use crate::time::period::Period;
use crate::time::schedule::schedule::{Schedule, ScheduleGenerator};


// ─────────────────────────────────────────────────────────────────────────────
// GenericLegCharacters
// ─────────────────────────────────────────────────────────────────────────────

pub struct GenericLegCharacters {
    compounding: Compounding,
    day_counter: DayCounter,
    schedule: Schedule,
}

impl GenericLegCharacters {
    pub fn new(
        compounding: Compounding,
        day_counter: DayCounter,
        schedule: Schedule,
    ) -> Self {
        Self { compounding, day_counter, schedule }
    }

    pub fn compounding(&self) -> &Compounding { &self.compounding }
    pub fn day_counter(&self) -> &DayCounter  { &self.day_counter }
    pub fn schedule(&self)    -> &Schedule    { &self.schedule }

    pub fn maturity_date(&self) -> NaiveDate {
        self.schedule.schedule_periods().last().unwrap().payment_date()
    }

    pub fn len(&self) -> usize { self.schedule.len() }
}


// ─────────────────────────────────────────────────────────────────────────────
// LegCharacters trait
// ─────────────────────────────────────────────────────────────────────────────

pub trait LegCharacters: Send + Sync {
    fn reference_curve_name(&self) -> Option<&String>;
    fn generic_characters(&self) -> &GenericLegCharacters;
    fn max_date(&self) -> NaiveDate;

    fn evaluate_flow(
        &self,
        i: usize,
        forward_curve_opt: Option<&Arc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> f64;
}


// ─────────────────────────────────────────────────────────────────────────────
// LegCharactersSetter
// ─────────────────────────────────────────────────────────────────────────────

pub struct LegCharactersSetter {
    fixed_rate: f64,
    spread: f64,
    leverage: f64,
}

impl LegCharactersSetter {
    pub fn new() -> Self {
        Self { fixed_rate: 0.0, spread: 0.0, leverage: 1.0 }
    }

    pub fn fixed_rate(&self) -> f64 { self.fixed_rate }
    pub fn set_fixed_rate(&mut self, v: f64) { self.fixed_rate = v; }

    pub fn spread(&self) -> f64 { self.spread }
    pub fn set_spread(&mut self, v: f64) { self.spread = v; }

    pub fn leverage(&self) -> f64 { self.leverage }
    pub fn set_leverage(&mut self, v: f64) { self.leverage = v; }
}

impl Default for LegCharactersSetter {
    fn default() -> Self { Self::new() }
}


// ─────────────────────────────────────────────────────────────────────────────
// GenericLegCharactersGenerator
// ─────────────────────────────────────────────────────────────────────────────

pub struct GenericLegCharactersGenerator {
    calendar:              Arc<dyn HolidayCalendar>,
    fixing_calendar:       Arc<dyn HolidayCalendar>,
    payment_calendar:      Arc<dyn HolidayCalendar>,
    schedule_generator:    Arc<ScheduleGenerator>,
    day_counter_generator: Arc<DayCounterGenerator>,
    compounding:           Compounding,
    setter:                LegCharactersSetter,
}

impl GenericLegCharactersGenerator {
    pub fn new(
        calendar:              Arc<dyn HolidayCalendar>,
        fixing_calendar:       Arc<dyn HolidayCalendar>,
        payment_calendar:      Arc<dyn HolidayCalendar>,
        schedule_generator:    Arc<ScheduleGenerator>,
        day_counter_generator: Arc<DayCounterGenerator>,
        compounding:           Compounding,
        setter:                LegCharactersSetter,
    ) -> Self {
        Self {
            calendar,
            fixing_calendar,
            payment_calendar,
            schedule_generator,
            day_counter_generator,
            compounding,
            setter,
        }
    }

    pub fn calendar(&self)              -> &Arc<dyn HolidayCalendar>  { &self.calendar }
    pub fn fixing_calendar(&self)       -> &Arc<dyn HolidayCalendar>  { &self.fixing_calendar }
    pub fn payment_calendar(&self)      -> &Arc<dyn HolidayCalendar>  { &self.payment_calendar }
    pub fn schedule_generator(&self)    -> &Arc<ScheduleGenerator>    { &self.schedule_generator }
    pub fn day_counter_generator(&self) -> &Arc<DayCounterGenerator>  { &self.day_counter_generator }
    pub fn compounding(&self)           -> &Compounding               { &self.compounding }
    pub fn setter(&self)                -> &LegCharactersSetter       { &self.setter }
}


// ─────────────────────────────────────────────────────────────────────────────
// LegCharactersGenerator trait
// ─────────────────────────────────────────────────────────────────────────────

pub trait LegCharactersGenerator {
    fn generic_characters_generator(&self) -> &GenericLegCharactersGenerator;

    // ── 透過 accessor 存取，不直接碰私有欄位 ──

    fn calendar(&self)              -> &Arc<dyn HolidayCalendar> { self.generic_characters_generator().calendar() }
    fn fixing_calendar(&self)       -> &Arc<dyn HolidayCalendar> { self.generic_characters_generator().fixing_calendar() }
    fn payment_calendar(&self)      -> &Arc<dyn HolidayCalendar> { self.generic_characters_generator().payment_calendar() }
    fn schedule_generator(&self)    -> &Arc<ScheduleGenerator>   { self.generic_characters_generator().schedule_generator() }
    fn day_counter_generator(&self) -> &Arc<DayCounterGenerator> { self.generic_characters_generator().day_counter_generator() }
    fn compounding(&self)           -> &Compounding              { self.generic_characters_generator().compounding() }
    fn setter(&self)                -> &LegCharactersSetter      { self.generic_characters_generator().setter() }

    /// 實作方提供：已有 Schedule 時如何建構 LegCharacters。
    fn generate_with_schedule(&self, schedule: Schedule) -> Arc<dyn LegCharacters>;

    fn generate_with_maturity_date(
        &self,
        trade_date: NaiveDate,
        maturity: NaiveDate,
    ) -> Result<Arc<dyn LegCharacters>, String> {
        let schedule = self
            .schedule_generator()
            .generate_with_maturity_date(
                trade_date, maturity,
                self.calendar(), self.fixing_calendar(), self.payment_calendar(),
            )
            .ok_or_else(|| "Failed to generate schedule from maturity date".to_string())?;
        Ok(self.generate_with_schedule(schedule))
    }

    fn generate_with_maturity_tenor(
        &self,
        trade_date: NaiveDate,
        maturity: Period,
    ) -> Result<Arc<dyn LegCharacters>, String> {
        let schedule = self
            .schedule_generator()
            .generate_from_maturity_tenor(
                trade_date, maturity,
                self.calendar(), self.fixing_calendar(), self.payment_calendar(),
            )
            .ok_or_else(|| "Failed to generate schedule from maturity tenor".to_string())?;
        Ok(self.generate_with_schedule(schedule))
    }
}
