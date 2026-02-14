use std::rc::Rc;

use chrono::NaiveDate;

use crate::interestrate::compounding::Compounding;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::daycounter::daycounter::{DayCounter, DayCounterGenerator};
use crate::time::period::Period;
use crate::time::schedule::schedule::Schedule;
use crate::time::schedule::schedule::ScheduleGenerator;

// ── 移除：不再需要 `use std::cell::Cell`

// ─────────────────────────────────────────────
// GenericLegCharacters
// ─────────────────────────────────────────────

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
    ) -> GenericLegCharacters {
        GenericLegCharacters {
            compounding,
            day_counter,
            schedule,
        }
    }

    pub fn compounding(&self) -> &Compounding {
        &self.compounding
    }

    pub fn day_counter(&self) -> &DayCounter {
        &self.day_counter
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn maturity_date(&self) -> NaiveDate {
        self.schedule
            .schedule_periods()
            .last()
            .unwrap()
            .payment_date()
    }

    pub fn len(&self) -> usize {
        self.schedule().len()
    }
}

// ─────────────────────────────────────────────
// LegCharacters trait
// ─────────────────────────────────────────────

pub trait LegCharacters {
    fn reference_curve_name(&self) -> Option<&String>;

    fn generic_characters(&self) -> &GenericLegCharacters;

    fn max_date(&self) -> NaiveDate;

    fn evaluate_flow(
        &self,
        i: usize,
        forward_curve_opt: Option<&Rc<dyn InterestRateCurve>>,
        pricing_condition: &PricingCondition,
    ) -> f64;
}

// ─────────────────────────────────────────────
// LegCharactersSetter
// 改進：移除 Cell<f64>，改用普通欄位 + &mut self setter
// ─────────────────────────────────────────────

pub struct LegCharactersSetter {
    fixed_rate: f64,
    spread: f64,
    leverage: f64,
}

impl LegCharactersSetter {
    pub fn new() -> LegCharactersSetter {
        LegCharactersSetter {
            fixed_rate: 0.0,
            spread: 0.0,
            leverage: 1.0,
        }
    }

    pub fn fixed_rate(&self) -> f64 {
        self.fixed_rate
    }

    pub fn set_fixed_rate(&mut self, fixed_rate: f64) {
        self.fixed_rate = fixed_rate;
    }

    pub fn spread(&self) -> f64 {
        self.spread
    }

    pub fn set_spread(&mut self, spread: f64) {
        self.spread = spread;
    }

    pub fn leverage(&self) -> f64 {
        self.leverage
    }

    pub fn set_leverage(&mut self, leverage: f64) {
        self.leverage = leverage;
    }
}

impl Default for LegCharactersSetter {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
// GenericLegCharactersGenerator
// 改進：加上 accessor methods，避免外部直接存取私有欄位
// ─────────────────────────────────────────────

pub struct GenericLegCharactersGenerator {
    calendar: Rc<dyn HolidayCalendar>,
    fixing_calendar: Rc<dyn HolidayCalendar>,
    payment_calendar: Rc<dyn HolidayCalendar>,
    schedule_generator: Rc<ScheduleGenerator>,
    day_counter_generator: Rc<DayCounterGenerator>,
    compounding: Compounding,
    setter: LegCharactersSetter,
}

impl GenericLegCharactersGenerator {
    pub fn new(
        calendar: Rc<dyn HolidayCalendar>,
        fixing_calendar: Rc<dyn HolidayCalendar>,
        payment_calendar: Rc<dyn HolidayCalendar>,
        schedule_generator: Rc<ScheduleGenerator>,
        day_counter_generator: Rc<DayCounterGenerator>,
        compounding: Compounding,
        setter: LegCharactersSetter,
    ) -> GenericLegCharactersGenerator {
        GenericLegCharactersGenerator {
            calendar,
            fixing_calendar,
            payment_calendar,
            schedule_generator,
            day_counter_generator,
            compounding,
            setter,
        }
    }

    // ── 新增 accessor，封裝私有欄位 ──

    pub fn calendar(&self) -> &Rc<dyn HolidayCalendar> {
        &self.calendar
    }

    pub fn fixing_calendar(&self) -> &Rc<dyn HolidayCalendar> {
        &self.fixing_calendar
    }

    pub fn payment_calendar(&self) -> &Rc<dyn HolidayCalendar> {
        &self.payment_calendar
    }

    pub fn schedule_generator(&self) -> &Rc<ScheduleGenerator> {
        &self.schedule_generator
    }

    pub fn day_counter_generator(&self) -> &Rc<DayCounterGenerator> {
        &self.day_counter_generator
    }

    pub fn compounding(&self) -> &Compounding {
        &self.compounding
    }

    pub fn setter(&self) -> &LegCharactersSetter {
        &self.setter
    }
}

// ─────────────────────────────────────────────
// LegCharactersGenerator trait
// 改進：
//   1. default methods 改透過 accessor 存取欄位
//   2. 新增 generate_with_schedule，消除兩個 generate_with_maturity_* 的重複邏輯
//   3. generate_with_maturity_* 回傳 Result，移除 unwrap
// ─────────────────────────────────────────────

pub trait LegCharactersGenerator {
    fn generic_characters_generator(&self) -> &GenericLegCharactersGenerator;

    // ── 透過 accessor 存取，不再直接碰私有欄位 ──

    fn calendar(&self) -> &Rc<dyn HolidayCalendar> {
        self.generic_characters_generator().calendar()
    }

    fn fixing_calendar(&self) -> &Rc<dyn HolidayCalendar> {
        self.generic_characters_generator().fixing_calendar()
    }

    fn payment_calendar(&self) -> &Rc<dyn HolidayCalendar> {
        self.generic_characters_generator().payment_calendar()
    }

    fn schedule_generator(&self) -> &Rc<ScheduleGenerator> {
        self.generic_characters_generator().schedule_generator()
    }

    fn day_counter_generator(&self) -> &Rc<DayCounterGenerator> {
        self.generic_characters_generator().day_counter_generator()
    }

    fn compounding(&self) -> &Compounding {
        self.generic_characters_generator().compounding()
    }

    fn setter(&self) -> &LegCharactersSetter {
        self.generic_characters_generator().setter()
    }

    // ── 新增：讓具體型別實作「已有 Schedule 時的建構邏輯」
    //    這樣 generate_with_maturity_* 的重複程式碼就可以統一在這裡處理 ──
    fn generate_with_schedule(&self, schedule: Schedule) -> Rc<dyn LegCharacters>;

    // ── 改進：回傳 Result，讓呼叫端自行處理錯誤 ──

    fn generate_with_maturity_date(
        &self,
        trade_date: NaiveDate,
        maturity: NaiveDate,
    ) -> Result<Rc<dyn LegCharacters>, String> {
        let schedule = self
            .schedule_generator()
            .generate_with_maturity_date(
                trade_date,
                maturity,
                self.calendar(),
                self.fixing_calendar(),
                self.payment_calendar(),
            )
            .map_err(|e| format!("Failed to generate schedule: {e}"))?;

        Ok(self.generate_with_schedule(schedule))
    }

    fn generate_with_maturity_tenor(
        &self,
        trade_date: NaiveDate,
        maturity: Period,
    ) -> Result<Rc<dyn LegCharacters>, String> {
        let schedule = self
            .schedule_generator()
            .generate_from_maturity_tenor(
                trade_date,
                maturity,
                self.calendar(),
                self.fixing_calendar(),
                self.payment_calendar(),
            )
            .map_err(|e| format!("Failed to generate schedule: {e}"))?;

        Ok(self.generate_with_schedule(schedule))
    }
}
