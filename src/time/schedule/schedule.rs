
use std::rc::Rc;

use chrono::NaiveDate;

use super::calculationperiodgenerator::CalculationPeriodGenerator;
use super::super::calendar::holidaycalendar::HolidayCalendar;
use super::super::period::Period;
use super::relativedategenerator::RelativeDateGenerator;
use super::scheduleperiod::SchedulePeriod;

#[derive(Clone)]
pub struct ScheduleGenerator {
    calculation_period_generator: CalculationPeriodGenerator,
    fixing_date_generator: RelativeDateGenerator,
    payment_date_generator: RelativeDateGenerator
}

pub struct Schedule {
    horizon: NaiveDate,
    maturity: NaiveDate,
    schedule_periods: Vec<SchedulePeriod>,
    generator: ScheduleGenerator,
    calendar: Rc<dyn HolidayCalendar>,
    fixing_calendar: Rc<dyn HolidayCalendar>,
    payment_calendar: Rc<dyn HolidayCalendar>
}

impl Schedule {
    pub fn new(horizon: NaiveDate,
               maturity: NaiveDate,
               schedule_periods: Vec<SchedulePeriod>,
               generator: ScheduleGenerator,
               calendar: Rc<dyn HolidayCalendar>,
               fixing_calendar: Rc<dyn HolidayCalendar>,
               payment_calendar: Rc<dyn HolidayCalendar>) -> Schedule {
        Schedule {
            horizon: horizon,
            maturity: maturity,
            schedule_periods: schedule_periods, 
            generator: generator,
            calendar: calendar,
            fixing_calendar: fixing_calendar,
            payment_calendar: payment_calendar
        }
    }

    pub fn horizon(&self) -> NaiveDate {
        self.horizon
    }

    pub fn maturity(&self) -> NaiveDate {
        self.maturity
    }

    pub fn schedule_periods(&self) -> &Vec<SchedulePeriod> {
        &self.schedule_periods
    }

    pub fn generator(&self) -> &ScheduleGenerator {
        &self.generator
    }

    pub fn calendar(&self) -> &Rc<dyn HolidayCalendar> {
        &self.calendar
    }

    pub fn fixing_calendar(&self) -> &Rc<dyn HolidayCalendar> {
        &self.fixing_calendar
    }

    pub fn payment_calendar(&self) -> &Rc<dyn HolidayCalendar> {
        &self.payment_calendar
    }

    pub fn len(&self) -> usize {
        self.schedule_periods.len()
    }
}


impl ScheduleGenerator {
    pub fn new(calculation_period_generator: CalculationPeriodGenerator,
               fixing_date_generator: RelativeDateGenerator,
               payment_date_generator: RelativeDateGenerator) -> ScheduleGenerator {
        ScheduleGenerator {
            calculation_period_generator: calculation_period_generator,
            fixing_date_generator: fixing_date_generator,
            payment_date_generator: payment_date_generator
        }
    }

    pub fn calculation_period_generator(&self) -> &CalculationPeriodGenerator {
        &self.calculation_period_generator
    }

    pub fn fixing_date_generator(&self) -> &RelativeDateGenerator {
        &self.fixing_date_generator
    }
    pub fn payment_date_generator(&self) -> &RelativeDateGenerator {
        &self.payment_date_generator
    }

    pub fn generate_with_maturity_date(&self,
                                       horizon: NaiveDate,
                                       maturity: NaiveDate,
                                       calendar:  &Rc<dyn HolidayCalendar>,
                                       fixing_calendar: &Rc<dyn HolidayCalendar>,
                                       payment_calendar: &Rc<dyn HolidayCalendar>) -> Option<Schedule> {
        let calculation_period_opt = self.calculation_period_generator.generate_from_maturity_date(calendar, horizon, maturity);
        if calculation_period_opt.is_none() {
            return  None;
        }
        let calculation_periods = calculation_period_opt.unwrap();
        let fixing_dates = self.fixing_date_generator.generate(&calculation_periods, fixing_calendar);
        let payment_dates = self.payment_date_generator.generate(&calculation_periods, payment_calendar);
        let mut schedule_periods: Vec<SchedulePeriod> = Vec::new();
        for i in 0..calculation_periods.len() {
            schedule_periods.push(SchedulePeriod::new(fixing_dates[i], calculation_periods[i], payment_dates[i]));
        }
        let schedule = Schedule::new(horizon, maturity, schedule_periods, self.clone(), calendar.clone(), fixing_calendar.clone(), payment_calendar.clone());
        Some(schedule)
    }

    pub fn generate_from_maturity_tenor(&self,
                                        horizon: NaiveDate,
                                        maturity: Period,
                                        calendar:  &Rc<dyn HolidayCalendar>,
                                        fixing_calendar: &Rc<dyn HolidayCalendar>,
                                        payment_calendar: &Rc<dyn HolidayCalendar>) -> Option<Schedule> {
        let start_date = calendar.shift_n_business_day(horizon, self.calculation_period_generator.start_lag());
        let maturity_date = self.calculation_period_generator.mat_adjuster().from_tenor_to_date(start_date, maturity, calendar);
        self.generate_with_maturity_date(horizon, maturity_date, calendar, fixing_calendar, payment_calendar)
    }
}
