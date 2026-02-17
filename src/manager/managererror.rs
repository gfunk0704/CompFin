use std::fmt::Display;

use serde::de::{
    self,
    Error
};
use serde::Deserialize;

use crate::time::daycounter::daycounter::DayCounterGenerationError;
use crate::time::period::ParsePeriodError;

#[derive(Debug)]
pub enum ManagerError {
    DayCounterGenerationError(DayCounterGenerationError),
    IOError(std::io::Error),
    JsonParseError(serde_json::Error),
    NameNotFoundError(String),
    TenorParseError(ParsePeriodError)
}

impl ManagerError {
    pub fn from_json_or_json_parse_error<T>(json_value: serde_json::Value) -> Result<T, Self>
    where
        T: for<'a> Deserialize<'a>
    {
        serde_json::from_value(json_value).map_err(ManagerError::JsonParseError)
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

    pub fn map_elem_not_found(name: &String) -> ManagerError {
        ManagerError::NameNotFoundError(name.to_owned())
    }
}

/// # 變更說明
/// 原本有一個 `to_string(&self) -> String` 的 inherent method，
/// 其實作邏輯已移入 `Display::fmt`。
///
/// 舊的 `Display::fmt` 寫法：`write!(f, "{}", self.to_string())` 是循環呼叫
/// （`Display::fmt` → `to_string()` → `Display::fmt` → ...），只是碰巧
/// 因為 inherent method 優先順序才沒有無限遞迴。移除 inherent method 後
/// 直接在 `Display::fmt` 裡實作，更清晰也更安全。
impl Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManagerError::IOError(e) => write!(f, "{}", e),
            ManagerError::JsonParseError(e) => write!(f, "{}", e),
            ManagerError::NameNotFoundError(name) => write!(f, "key '{}' not found", name),
            ManagerError::TenorParseError(e) => write!(f, "{}", e),
            ManagerError::DayCounterGenerationError(e) => write!(f, "{}", e),
        }
    }
}

/// # 新增：實作 `std::error::Error`
/// 使 `ManagerError` 成為標準錯誤型別，可與 `?` operator、
/// `Box<dyn Error>`、`anyhow::Error` 等無縫整合。
/// `source()` 回傳內部錯誤，保留完整的錯誤鏈。
impl std::error::Error for ManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ManagerError::IOError(e) => Some(e),
            ManagerError::JsonParseError(e) => Some(e),
            ManagerError::TenorParseError(e) => Some(e),
            ManagerError::DayCounterGenerationError(_) => None,
            ManagerError::NameNotFoundError(_) => None,
        }
    }
}
