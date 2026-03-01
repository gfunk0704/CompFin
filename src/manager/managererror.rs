use serde::Deserialize;
use thiserror::Error;

use crate::time::daycounter::daycounter::DayCounterGenerationError;
use crate::time::period::ParsePeriodError;

#[derive(Debug, Error)]
pub enum ManagerError {
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    JsonParse(#[from] serde_json::Error),

    #[error("key '{0}' not found")]
    NotFound(String),

    #[error(transparent)]
    TenorParse(#[from] ParsePeriodError),

    #[error(transparent)]
    DayCounterGeneration(#[from] DayCounterGenerationError),

    #[error("invalid value: {0}")]
    InvalidValue(String),
}

/// JSON Value 反序列化的便利函式。
///
/// 取代原本的 `ManagerError::from_json_or_json_parse_error`，
/// 不再需要透過 `ManagerError` 的 inherent method 呼叫，語意更清晰。
///
/// # 範例
/// ```rust
/// let props: MyJsonProp = parse_json_value(json_value)?;
/// ```
pub fn parse_json_value<T>(json_value: serde_json::Value) -> Result<T, ManagerError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(json_value).map_err(ManagerError::from)
}
