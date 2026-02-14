use std::fmt;
use std::rc::Rc;

use chrono::{
    Datelike, 
    NaiveDate
};

use serde::{
    Serialize,
    Deserialize
};
use serde::de;

use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::period::Period;

fn unadjust(d: NaiveDate, _calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
    d
}

fn following(d: NaiveDate, calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
    calendar.next_business_day(d)
}

fn preceding(d: NaiveDate, calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
    calendar.previous_business_day(d)
}

fn modified_following(d: NaiveDate, calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
    let eom = calendar.last_business_day_of_month(d.year(),d.month());
    if d > eom {
        eom
    } else {
        calendar.next_business_day(d)
    }
}

fn modified_preceding(d: NaiveDate, calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
    let fom = calendar.first_business_day_of_month(d.year(),d.month());
    if d < fom {
        fom
    } else {
        calendar.previous_business_day(d)
    }
}

fn half_month_modified_following(d: NaiveDate, calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
    let adjusted = calendar.next_business_day(d);
    if (adjusted.month() != d.month()) ||
       ((d.day() <= 15) && (adjusted.day() > 15)) {
        calendar.previous_business_day(d)
    } else {
        adjusted
    }
}

fn nearest(d: NaiveDate, calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
    let previous_day = calendar.previous_business_day(d);
    let next_day = calendar.next_business_day(d);
    if (next_day - d).num_days() < (d - previous_day).num_days() {
        next_day
    } else {
        previous_day
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum BusinessDayConvention {
    Unadjusted,
    Following,
    Preceding,
    ModifiedFollowing,
    ModifiedPreceding,
    HalfMonthModifiedFollowing,
    Nearest
}

#[derive(Clone, Copy)]
pub struct BusinessDayAdjuster {
    convention: BusinessDayConvention,
    eom: bool,
    adjuster: fn(NaiveDate, &Rc<dyn HolidayCalendar>) -> NaiveDate 
}

impl BusinessDayAdjuster {
    pub fn new(convention: BusinessDayConvention, eom: bool) -> BusinessDayAdjuster {
        let adjuster = match convention {
            BusinessDayConvention::Unadjusted => unadjust,
            BusinessDayConvention::Following => following,
            BusinessDayConvention::Preceding => preceding,
            BusinessDayConvention::ModifiedFollowing => modified_following,
            BusinessDayConvention::ModifiedPreceding => modified_preceding,
            BusinessDayConvention::HalfMonthModifiedFollowing => half_month_modified_following,
            BusinessDayConvention::Nearest => nearest
        };
        BusinessDayAdjuster {convention: convention, eom: eom, adjuster: adjuster}
    }

    pub fn convention(&self) -> BusinessDayConvention {
        self.convention
    }

    pub fn eom(&self) -> bool {
        self.eom
    }

    pub fn adjust(&self, d: NaiveDate, calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
        if calendar.is_holiday(d) {
            (self.adjuster)(d, calendar)
        } else {
            d
        }
    }

    pub fn from_tenor_to_date(&self, 
                              horizon: NaiveDate, 
                              tenor: Period, 
                              calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
        if self.eom {
            if calendar.last_business_day_of_month(horizon.year(), horizon.month()) == horizon {
                let d = horizon + tenor;
                calendar.last_business_day_of_month(d.year(), d.month())
            } else {
                self.from_tenor_to_date_without_eom_rule(horizon, tenor, calendar)
            }
        } else {
            self.from_tenor_to_date_without_eom_rule(horizon, tenor, calendar)
        }
    }

    fn from_tenor_to_date_without_eom_rule(&self, 
                                           horizon: NaiveDate, 
                                           tenor: Period, 
                                           calendar: &Rc<dyn HolidayCalendar>) -> NaiveDate {
        let d = horizon + tenor;
        if calendar.is_holiday(d) {
            self.adjust(d, calendar)
        } else {
            d
        }
    }
}

impl<'de> de::Deserialize<'de> for BusinessDayAdjuster {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        enum Field { Convention, Eom }

        // This part could also be generated independently by:
        //
        //    #[derive(Deserialize)]
        //    #[serde(field_identifier, rename_all = "lowercase")]
        //    enum Field { Secs, Nanos }
        impl<'de> de::Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: de::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> de::Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`convention` or `eom`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "convention" => Ok(Field::Convention),
                            "eom" => Ok(Field::Eom),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct BusinessDayAdjusterVisitor;

        impl<'de> de::Visitor<'de> for BusinessDayAdjusterVisitor {
            type Value = BusinessDayAdjuster;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct BusinessDayAdjuster")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<BusinessDayAdjuster, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let convention = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let eom = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(BusinessDayAdjuster::new(convention, eom))
            }

            fn visit_map<V>(self, mut map: V) -> Result<BusinessDayAdjuster, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut convention = None;
                let mut eom = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Convention => {
                            if convention.is_some() {
                                return Err(de::Error::duplicate_field("convention"));
                            }
                            convention = Some(map.next_value()?);
                        }
                        Field::Eom => {
                            if eom.is_some() {
                                return Err(de::Error::duplicate_field("eom"));
                            }
                            eom = Some(map.next_value()?);
                        }
                    }
                }
                let convention = convention.ok_or_else(|| de::Error::missing_field("convention"))?;
                let eom = eom.ok_or_else(|| de::Error::missing_field("eom"))?;
                Ok(BusinessDayAdjuster::new(convention, eom))
            }
        }

        const FIELDS: &[&str] = &["convention", "eom"];
        deserializer.deserialize_struct("BusinessDayAdjuster", FIELDS, BusinessDayAdjusterVisitor)
    }
}

