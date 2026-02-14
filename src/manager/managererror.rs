use std::fmt::Display;

use serde::{de::{
    self, 
    Error
}, Deserialize};

use crate::time::daycounter::daycounter::DayCounterGenerationError;

use super::super::time::period::ParsePeriodError;

#[derive(Debug)]
pub enum ManagerError {
    DayCounterGenerationError(DayCounterGenerationError),
    IOError(std::io::Error),
    JsonParseError(serde_json::Error),
    NameNotFoundError(String),
    TenorParseError(ParsePeriodError)
}

impl ManagerError {
    pub fn from_json_or_json_parse_error <T> (json_value: serde_json::Value) -> Result<T, Self> 
        where T : for<'a> Deserialize<'a> {
        let obj_result: Result<T, serde_json::Error> = serde_json::from_value(json_value);
        obj_result.map_or_else(
            |err| Err(ManagerError::JsonParseError(err)), 
            |elem| Ok(elem)
        )
    }

    pub fn json_missing_field(field: &'static str) -> ManagerError {
        ManagerError::JsonParseError(serde_json::Error::missing_field(field))
    }

    pub fn json_invalid_length(len: usize, exp: &dyn de::Expected) -> ManagerError {
        ManagerError::JsonParseError(serde_json::Error::invalid_length(len, exp))
    }

    pub fn json_invalid_type(unexp: de::Unexpected, exp: &dyn de::Expected) -> ManagerError {
        ManagerError::JsonParseError(serde_json::Error::invalid_type(unexp, exp))
    }

    pub fn json_invalid_value(unexp: de::Unexpected, exp: &dyn de::Expected) -> ManagerError {
        ManagerError::JsonParseError(serde_json::Error::invalid_value(unexp, exp))
    }

    pub fn map_elem_not_found(name: &String) -> ManagerError{
        ManagerError::NameNotFoundError(name.to_owned())
    }

    pub fn to_string(&self) -> String {
        match self {
            ManagerError::IOError(error) => error.to_string(),
            ManagerError::JsonParseError(error) => error.to_string(),
            ManagerError::NameNotFoundError(name) => {
                let mut result = "key '".to_string();
                result.push_str(name.as_str());
                result.push_str("' not found");
                result
            },
            ManagerError::TenorParseError(error) => error.to_string(),
            ManagerError::DayCounterGenerationError(error) => error.to_string()
        }
    }
}

impl Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}