use std::ops::{
    Add,
    Sub
};

use chrono::NaiveDate;

use super::super::market::currency::Currency;


pub struct NPV {
    currency: Currency,
    amount: f64,
    settlement_date: NaiveDate
}


pub enum NPVArithmeticOperationError {
    CurrenciesMismatched,
    SettlementDateMismatched
}

impl NPV {
    pub fn new(currency: Currency, 
               amount: f64,
               settlement_date: NaiveDate) -> NPV {
        NPV {
            currency: currency,
            amount: amount,
            settlement_date: settlement_date
        }
    }

    pub fn currency(&self) -> &Currency {
        &self.currency
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn settlement_date(&self) -> &NaiveDate {
        &self.settlement_date
    }
}


fn arithmetic_operation(lhs: NPV, rhs: NPV, op: fn(f64, f64) -> f64) -> Result<NPV, NPVArithmeticOperationError> {
    if lhs.currency.code() != rhs.currency.code() {
        Err(NPVArithmeticOperationError::CurrenciesMismatched)
    } else if lhs.settlement_date != rhs.settlement_date {
        Err(NPVArithmeticOperationError::SettlementDateMismatched)
    } else {
        Ok(NPV::new(lhs.currency, op(lhs.amount, rhs.amount), lhs.settlement_date))
    }
}

impl Add<Self> for NPV {
    type Output = Result<Self, NPVArithmeticOperationError>;

    fn add(self, rhs: Self) -> Self::Output {
        arithmetic_operation(self, rhs, Add::add)
    }
}


impl Sub<Self> for NPV {
    type Output = Result<Self, NPVArithmeticOperationError>;

    fn sub(self, rhs: Self) -> Self::Output {
        arithmetic_operation(self, rhs, Sub::sub)
    }
}