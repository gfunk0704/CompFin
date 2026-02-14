
use chrono::NaiveDate;


#[derive(Clone, Copy)]
pub struct CalculationPeriod {
    start_date: NaiveDate,
    end_date: NaiveDate
}

impl CalculationPeriod {
    pub fn new(start_date: NaiveDate, end_date: NaiveDate) -> CalculationPeriod {
        CalculationPeriod {start_date: start_date, end_date: end_date}
    }

    pub fn start_date(&self) -> NaiveDate {
        self.start_date
    }

    pub fn end_date(&self) -> NaiveDate {
        self.end_date
    }
}

pub struct SchedulePeriod {
    fixing_date: NaiveDate,
    calculation_period: CalculationPeriod,
    payment_date: NaiveDate
}

impl SchedulePeriod {
    pub fn new(fixing_date: NaiveDate, 
               calculation_period: CalculationPeriod,
               payment_date: NaiveDate) -> SchedulePeriod {
        SchedulePeriod {fixing_date: fixing_date, calculation_period: calculation_period, payment_date: payment_date}
    }

    pub fn fixing_date(&self) -> NaiveDate {
        self.fixing_date
    }

    pub fn calculation_period(&self) -> CalculationPeriod {
        self.calculation_period
    }

    pub fn payment_date(&self) -> NaiveDate {
        self.payment_date
    }
}