
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use chrono::NaiveDate;

use crate::instrument::leg::fixingratecalculator::fixingratecalculator::{
    FixingRateCalculator, 
    FixingRateCalculatorGenerator,
    FixingRateType
};
use crate::interestrate::index::interestrateindex::{
    InterestRateIndex,
    InterestRateIndexType
};
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::period::TimeUnit;
use crate::time::schedule::schedule::Schedule;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FixingConvention {
    Advance,
    Arrear
}


#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MissingFixingHandler {
    Null,
    PreviousFixing
}

fn null_missing_fixing_handler_impl (index: &Rc<dyn InterestRateIndex>,
                                     d: NaiveDate) -> f64 {
    index.past_fixings()[d]
}

fn previous_missing_fixing_handler_impl (index: &Rc<dyn InterestRateIndex>,
                                         d: NaiveDate) -> f64 {
    let borrow = index.past_fixings();
    let fixing_opt = borrow.get(&d);
    if fixing_opt.is_some() {
        *fixing_opt.unwrap()
    } else {
        let mut fixing_dates: Vec<NaiveDate> = borrow.keys().collect();
        fixing_dates.sort();
        let rhs_pos = fixing_dates.partition_point(|&fixing_date| fixing_date < d);
        let traget_date = fixing_dates[rhs_pos - 1];
        borrow[traget_date]
    }
}

fn standard_forward_accrual_impl (forward_curve: &Rc<dyn InterestRateCurve>,
                                  index: &Rc<dyn InterestRateIndex>,
                                  daily_period_deltails_list: &Vec<(NaiveDate, f64)>,
                                  _end_date: NaiveDate) -> f64 {
    let mut accrual = 1.0;
    
    for (fixing_date, tau) in daily_period_deltails_list.iter() {
        let r = index.projection_rate(*fixing_date, forward_curve);
        accrual *= 1.0 + r * tau;
    }

   accrual
}

fn arbitrage_free_accrual_impl (forward_curve: &Rc<dyn InterestRateCurve>,
                                _index: &Rc<dyn InterestRateIndex>,
                                daily_period_deltails_list: &Vec<(NaiveDate, f64)>,
                                end_date: NaiveDate) -> f64 {
    let discount_start = forward_curve.discount(daily_period_deltails_list[0].0);
    let discount_end = forward_curve.discount(end_date);
    discount_start / discount_end - 1.0
}

pub struct DailyCompoundedRateCalculator {
    index: Rc<dyn InterestRateIndex>,
    fixing_convention: FixingConvention,
    daily_period_deltails_list: Vec<Vec<(NaiveDate, f64)>>,
    end_date: NaiveDate,
    calculation_period_year_fractions: Vec<f64>,
    apply_arbitrage_free_projection: Cell<bool>,
    arbitrage_free_projection_applicable: bool,
    shift_days: u32,
    missing_fixing_handler: MissingFixingHandler,
    missing_fixing_handler_impl: fn(&Rc<dyn InterestRateIndex>, NaiveDate) -> f64
}

impl DailyCompoundedRateCalculator {
    pub fn new(index: Rc<dyn InterestRateIndex>,
               schedule: &Schedule,
               fixing_convention: FixingConvention,
               shift_days: u32,
               missing_fixing_handler: MissingFixingHandler) -> DailyCompoundedRateCalculator {

        assert!((index.tenor().number() == 1) && (index.tenor().unit() == TimeUnit::Days),
                "index tenor must be '1D'");
        
        assert!(index.index_type() == InterestRateIndexType::TermRate,
                "index type must be 'TermRate'");

        let mut daily_period_deltails_list: Vec<Vec<(NaiveDate, f64)>> = Vec::new();
        let mut calculation_period_year_fractions: Vec<f64> = Vec::new();
        let shift_days: i32 = -(shift_days as i32);
        let compounded_day_counter = index.day_counter();
        let get_fixing_date = match fixing_convention {
            FixingConvention::Advance => |d1:NaiveDate, _d2: NaiveDate| d1,
            FixingConvention::Arrear => |_d1:NaiveDate, d2: NaiveDate| d2
        };

        for schedule_period in schedule.schedule_periods().iter() {
            let mut daily_period_deltails: Vec<(NaiveDate, f64)> = Vec::new();

            let calculation_period = schedule_period.calculation_period();
            let mut calculation_period_year_fraction = 0.0;
            let mut d = calculation_period.start_date();
            let mut fixing_date = schedule.
                    fixing_calendar().
                    shift_n_business_day(calculation_period.start_date(), shift_days);
            
            let fixing_end_date = schedule.
                fixing_calendar().
                shift_n_business_day(calculation_period.end_date(), shift_days);
        
            while fixing_date < fixing_end_date {
                let next_fixing_date = schedule.fixing_calendar().next_business_day(fixing_date);
                let next_d = schedule.calendar().next_business_day(d);
                let tau = compounded_day_counter.year_fraction(d, next_d);
                daily_period_deltails.push((get_fixing_date(fixing_date, next_fixing_date), tau));
                fixing_date = next_fixing_date;
                d = next_d;
                calculation_period_year_fraction += tau;
            }

            daily_period_deltails_list.push(daily_period_deltails);
            calculation_period_year_fractions.push(calculation_period_year_fraction);
        }

        let arbitrage_free_projection_applicable = (shift_days == 0) && (fixing_convention == FixingConvention::Advance);
       

        let missing_fixing_handler_impl = match missing_fixing_handler {
            MissingFixingHandler::Null => null_missing_fixing_handler_impl,
            MissingFixingHandler::PreviousFixing => previous_missing_fixing_handler_impl
        };

        DailyCompoundedRateCalculator {
            index: index,
            fixing_convention: fixing_convention,
            daily_period_deltails_list: daily_period_deltails_list,
            end_date: schedule.schedule_periods().last().unwrap().calculation_period().end_date(),
            calculation_period_year_fractions: calculation_period_year_fractions,
            arbitrage_free_projection_applicable: arbitrage_free_projection_applicable,
            apply_arbitrage_free_projection: Cell::new(arbitrage_free_projection_applicable),
            shift_days: shift_days as u32,
            missing_fixing_handler: missing_fixing_handler,
            missing_fixing_handler_impl: missing_fixing_handler_impl
        }
    }

    pub fn fixing_convention(&self) -> &FixingConvention {
        &self.fixing_convention
    }

    pub fn arbitrage_free_projection_applicable(&self) -> bool {
        self.arbitrage_free_projection_applicable
    }

    pub fn apply_arbitrage_free_projection(&self) -> bool {
        self.apply_arbitrage_free_projection.get()
    }

    pub fn set_apply_arbitrage_free_projection(&self, apply: bool) -> bool {
        if apply && self.arbitrage_free_projection_applicable {
            self.apply_arbitrage_free_projection.set(true);
        }
        self.apply_arbitrage_free_projection.get()
    }

    pub fn shift_days(&self) -> u32 {
        self.shift_days
    }

    pub fn missing_fixing_handler(&self) -> &MissingFixingHandler {
        &self.missing_fixing_handler
    }

    fn past_accrual(&self,
                    daily_period_deltails: &Vec<(NaiveDate, f64)>) -> f64 {
        let mut accrual = 1.0;
        for (d, tau) in daily_period_deltails.iter() {
            accrual *= 1.0 + (self.missing_fixing_handler_impl)(&self.index, *d) * tau;
        }
        accrual
    }
}


impl FixingRateCalculator for DailyCompoundedRateCalculator {
    fn index(&self) -> &Rc<dyn InterestRateIndex> {
        &self.index
    }

    fn fixing_rate_type(&self) -> FixingRateType {
        FixingRateType::DailyCompounding
    }

    fn relative_dates(&self,
                          i: usize) -> HashSet<NaiveDate> {
        let daily_period_deltails = self.daily_period_deltails_list.
            get(i).unwrap();
        
        let mut date_set: HashSet<NaiveDate> = daily_period_deltails.iter().map(|detial| detial.0).collect();
        date_set.insert(self.index.end_date(daily_period_deltails.last().unwrap().0));
        date_set
    }

    fn fixing(&self,
              i: usize,
              forward_curve: &Rc<dyn InterestRateCurve>,
              pricing_condition: &PricingCondition) -> f64 {
        let daily_period_deltails = self.daily_period_deltails_list.
            get(i).
            unwrap();
        
        let accrual_impl = if self.apply_arbitrage_free_projection() {
            arbitrage_free_accrual_impl
        } else {
            standard_forward_accrual_impl       
        };

        let accrual = if (*pricing_condition.horizon() < daily_period_deltails[0].0) ||
                              ((*pricing_condition.horizon() == daily_period_deltails[0].0) && *pricing_condition.estimate_horizon_index()) {
            
            (accrual_impl)(forward_curve, &self.index, daily_period_deltails, self.end_date)
        } else if (*pricing_condition.horizon() > daily_period_deltails[0].0) ||
                  ((*pricing_condition.horizon() == daily_period_deltails[0].0) && !(*pricing_condition.estimate_horizon_index())) {
            
            self.past_accrual(daily_period_deltails)
        } else {
           
            let mut forward_start_index = daily_period_deltails.partition_point(|&detial| detial.0 < *pricing_condition.horizon());
            forward_start_index += *pricing_condition.estimate_horizon_index() as usize;
            let past_accrual = self.past_accrual(&daily_period_deltails[..forward_start_index].to_vec());
            let forward_accrual = (accrual_impl)(forward_curve, &self.index, &daily_period_deltails[forward_start_index..].to_vec(), self.end_date);
            past_accrual * forward_accrual
        };

        (accrual - 1.0) / self.calculation_period_year_fractions[i]
    }
}


pub struct DailyCompoundedRateCalculatorGenerator {
    index: Rc<dyn InterestRateIndex>,
    fixing_convention: FixingConvention,
    shift_days: u32,
    missing_fixing_handler: MissingFixingHandler
}

impl DailyCompoundedRateCalculatorGenerator {
    pub fn new(index: Rc<dyn InterestRateIndex>,
               fixing_convention: FixingConvention,
               shift_days: u32,
               missing_fixing_handler: MissingFixingHandler) -> DailyCompoundedRateCalculatorGenerator {
        DailyCompoundedRateCalculatorGenerator {
            index: index,
            fixing_convention: fixing_convention,
            shift_days: shift_days,
            missing_fixing_handler: missing_fixing_handler
        }
    }

    pub fn fixing_convention(&self) -> &FixingConvention {
        &self.fixing_convention
    }

    pub fn shift_days(&self) -> u32 {
        self.shift_days
    }

    pub fn missing_fixing_handler(&self) -> &MissingFixingHandler {
        &self.missing_fixing_handler
    }
}

impl FixingRateCalculatorGenerator for DailyCompoundedRateCalculatorGenerator {
    fn index(&self) -> &Rc<dyn InterestRateIndex> {
        &self.index
    }

    fn fixing_rate_type(&self) -> FixingRateType {
        FixingRateType::DailyCompounding
    }
    
    fn generate(&self,
                schedule: &Schedule) -> Rc<dyn FixingRateCalculator> {
        Rc::new(DailyCompoundedRateCalculator::new(self.index.clone(),
                                                   schedule,
                                                   self.fixing_convention,
                                                   self.shift_days,
                                                   self.missing_fixing_handler))
    }
}