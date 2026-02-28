use std::sync::Arc;

use chrono::NaiveDate;
use serde::{
    Deserialize, 
    Serialize
};

use crate::market::currency::{Currency, CurrencyPair};
use crate::market::market::Market;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::period::Period;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ATMConvention {
    AtTheMoneyForward,
    DeltaNeutral
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeltaConvention {
    PipsSpot,
    PipsForward,
    PercentageSpot,
    PercentageForward
}

pub struct FxMatket {
    currency_pair: CurrencyPair,
    domestic_discount_curve_name: String,
    foreign_discount_curve_name: String,
    settlement_currency: Currency,
    settlement_days: u32,
    /// 比價日 calendar（通常只看 base currency 的 fixing calendar）
    expiry_calendar: Arc<dyn HolidayCalendar>,
    /// 交割 calendar（雙邊 banking calendar 的交集）
    settlement_calendar: Arc<dyn HolidayCalendar>,
    atm_convention: ATMConvention,
    short_term_delta_convention: DeltaConvention,
    long_term_delta_convention: DeltaConvention,
    max_short_term_tenor: Period
}


fn to_delta_convention(premium_in_delta: bool, spot_delta: bool) -> DeltaConvention {
    if premium_in_delta {
        if spot_delta {
            DeltaConvention::PipsSpot
        } else {
            DeltaConvention::PipsForward
        }
    } else {
        if spot_delta {
            DeltaConvention::PercentageSpot
        } else {
            DeltaConvention::PercentageForward
        }
    }
}


impl FxMatket {
    pub fn new(
        currency_pair: CurrencyPair,
        domestic_discount_curve_name: String,
        foreign_discount_curve_name: String,
        settlement_currency: Currency,
        settlement_days: u32,
        expiry_calendar: Arc<dyn HolidayCalendar>,
        settlement_calendar: Arc<dyn HolidayCalendar>,
        atm_convention: ATMConvention,
        premium_in_delta: bool,
        spot_delta_for_short_term: bool,
        spot_delta_for_long_term: bool,
        max_short_term_tenor: Period,
    ) -> Self {
        assert!(settlement_days <= 2, "For forex market, settlement days should be equal or less than 2.");

        Self {
            currency_pair,
            domestic_discount_curve_name,
            foreign_discount_curve_name,
            settlement_currency,
            settlement_days,
            expiry_calendar,
            settlement_calendar,
            atm_convention,
            short_term_delta_convention: to_delta_convention(premium_in_delta, spot_delta_for_short_term),
            long_term_delta_convention: to_delta_convention(premium_in_delta, spot_delta_for_long_term),
            max_short_term_tenor
        }
    }

    pub fn currency_pair(&self) -> &CurrencyPair {
        &self.currency_pair
    }

    pub fn domestic_discount_curve_name(&self) -> &String {
        &self.domestic_discount_curve_name
    }

    pub fn foreign_discount_curve_name(&self) -> &String {
        &self.foreign_discount_curve_name
    }

    pub fn settlement_currency(&self) -> &Currency {
        &self.settlement_currency
    }

    pub fn settlement_days(&self) -> u32 {
        self.settlement_days
    }

    pub fn atm_convention(&self) -> ATMConvention {
        self.atm_convention
    }

    pub fn short_term_delta_convention(&self) -> DeltaConvention {
        self.short_term_delta_convention
    }

    pub fn long_term_delta_convention(&self) -> DeltaConvention {
        self.long_term_delta_convention
    }

    pub fn max_short_term_tenor(&self) -> Period {
        self.max_short_term_tenor
    }
}


impl Market for FxMatket {
    fn discount_curve_name(&self) -> &String {
        self.domestic_discount_curve_name()
    }

    fn settlement_calendar(&self) -> Arc<dyn HolidayCalendar> {
        Arc::clone(&self.settlement_calendar)
    }

    fn expiry_calendar(&self) -> Arc<dyn HolidayCalendar> {
        Arc::clone(&self.expiry_calendar)
    }


    fn settlement_currency(&self) -> &Currency {
        self.currency_pair.ccy2()
    }

    fn settlement_days(&self) -> u32 {
        self.settlement_days
    }

    fn settlement_date(&self, horizon: NaiveDate) -> NaiveDate {
        match self.settlement_days {
            // T+0：當天結算
            0 => horizon,
            // T+1：直接用 settlement_calendar 推一個 BD（例如 USD/CAD）
            1 => self.settlement_calendar().shift_n_business_day(horizon, 1),
            // T+2：第一步用 expiry_calendar 推 overnight，
            // 第二步再用 settlement_calendar 推一個 BD 到 delivery
            // → 確保 delivery 當天雙邊清算系統均開市（Clark §1.5）
            _ => {
                let overnight = self.expiry_calendar().shift_n_business_day(horizon, 1);
                self.settlement_calendar().shift_n_business_day(overnight, 1)
            }
        }
    }
}