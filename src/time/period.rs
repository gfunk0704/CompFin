use std::fmt;
use std::ops::{Add, Sub};
use std::num::ParseIntError;

use chrono::{
    Datelike,
    Duration,
    NaiveDate
};

use crate::time::utility::days_of_month;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TimeUnit {
    Days,
    Weeks,
    Months,
    Years
}

impl TimeUnit {
    pub fn to_char(&self) -> char {
        match self {
            TimeUnit::Days => 'D',
            TimeUnit::Weeks => 'W',
            TimeUnit::Months => 'M',
            TimeUnit::Years => 'Y'
        }
    }
}

#[derive(Debug)]
pub enum ParsePeriodError {
    UnknownTimeUnit(char),
    Parse(ParseIntError)
}

/// # 變更說明
/// 實作 `std::fmt::Display`，取代原本的自定義 `to_string()` 方法。
/// 好處：
/// - `Display` 自動提供 `to_string()`（透過 `ToString` blanket impl），原有呼叫方不受影響。
/// - 與標準 `std::error::Error` trait 相容，可直接搭配 `?` operator 及 `anyhow` 等 error crate。
impl fmt::Display for ParsePeriodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParsePeriodError::UnknownTimeUnit(unit) => {
                write!(f, "unknown time unit '{}' found", unit)
            },
            ParsePeriodError::Parse(error) => {
                write!(f, "{}", error)
            }
        }
    }
}

/// # 變更說明
/// 實作 `std::error::Error`，使 `ParsePeriodError` 成為標準錯誤型別。
/// - 允許 `Box<dyn Error>` 與 `anyhow::Error` 等容器直接包裝。
/// - `source()` 回傳內部 `ParseIntError`，保留錯誤鏈。
impl std::error::Error for ParsePeriodError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParsePeriodError::Parse(e) => Some(e),
            ParsePeriodError::UnknownTimeUnit(_) => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Period {
    number: i32,
    unit: TimeUnit
}

impl Period {
    pub fn new(number: i32, unit: TimeUnit) -> Period {
        Period { number, unit }
    }

    pub fn days(number: i32) -> Period {
        Period::new(number, TimeUnit::Days)
    }

    pub fn weeks(number: i32) -> Period {
        Period::new(number, TimeUnit::Weeks)
    }

    pub fn months(number: i32) -> Period {
        Period::new(number, TimeUnit::Months)
    }

    pub fn years(number: i32) -> Period {
        Period::new(number, TimeUnit::Years)
    }

    /// # 變更說明
    /// 參數型別由 `String`（取得所有權）改為 `&str`（借用）。
    /// - 呼叫方不再需要 clone 或讓出字串所有權。
    /// - `String::len()`、`chars().nth()`、切片語法 `[..n]` 均適用於 `&str`，邏輯不變。
    pub fn parse(period_str: &str) -> Result<Period, ParsePeriodError> {
        // 原始：pub fn parse(period_str: String) -> Result<Period, ParsePeriodError>
        let last_index = period_str.len() - 1;
        let unit_chr = period_str.chars().nth(last_index).unwrap();
        let number_result = period_str[..last_index].parse::<i32>();
        if number_result.is_ok() {
            let number = number_result.unwrap();
            match unit_chr {
                'D' => Ok(Period::days(number)),
                'W' => Ok(Period::weeks(number)),
                'M' => Ok(Period::months(number)),
                'Y' => Ok(Period::years(number)),
                _ => Err(ParsePeriodError::UnknownTimeUnit(unit_chr))
            }
        } else {
            Err(ParsePeriodError::Parse(number_result.err().unwrap()))
        }
    }

    pub fn number(&self) -> i32 {
        self.number
    }

    pub fn unit(&self) -> TimeUnit {
        self.unit
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.number, self.unit.to_char())
    }
}

fn shift_months(horizon: NaiveDate, number: i32) -> NaiveDate {
    let total = horizon.month0() as i32 + number;
    let new_year = horizon.year() + total.div_euclid(12);
    let new_month = total.rem_euclid(12) as u32 + 1;
    let last = days_of_month(new_year, new_month);
    NaiveDate::from_ymd_opt(new_year, new_month, last.min(horizon.day())).unwrap()
}

fn shift_years(horizon: NaiveDate, number: i32) -> NaiveDate {
    let new_year = horizon.year() + number;
    let last = days_of_month(new_year, horizon.month());
    NaiveDate::from_ymd_opt(new_year, horizon.month(), last.min(horizon.day())).unwrap()
}

impl Add<Period> for NaiveDate {
    type Output = Self;

    fn add(self, period: Period) -> Self {
        match period.unit {
            TimeUnit::Days => self + Duration::days(period.number as i64),
            TimeUnit::Weeks => self + Duration::days(7 * period.number as i64),
            TimeUnit::Months => shift_months(self, period.number),
            TimeUnit::Years => shift_years(self, period.number)
        }
    }
}

impl Sub<Period> for NaiveDate {
    type Output = Self;

    fn sub(self, period: Period) -> Self {
        self + Period::new(-period.number, period.unit)
    }
}
