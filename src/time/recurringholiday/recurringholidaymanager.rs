use std::collections::HashMap;
use std::rc::Rc;

use chrono::Weekday;
use serde::Deserialize;
use serde_json;

use super::super::super::manager::managererror::ManagerError;
use super::super::recurringholiday::recurringholiday::RecurringHoliday;
use super::super::recurringholiday::weekendadjustment::WeekendAdjustment;
use super::super::recurringholiday::fixeddateholiday::FixedDateHoliday;
use super::super::recurringholiday::nthweekdayholiday::NthWeekdayHoliday;
use super::super::recurringholiday::lastweekdayholiday::LastWeekdayHoliday;
use super::super::recurringholiday::easterrelatedholiday::{
    EasterType,
    EasterRelatedHoliday
};





