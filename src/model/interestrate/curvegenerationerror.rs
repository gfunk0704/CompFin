use thiserror::Error;

#[derive(Debug, Error)]
pub enum CurveGenerationError {
    #[error("values length {values_len} does not match dates length {dates_len}")]
    LengthMismatch {
        values_len: usize,
        dates_len:  usize,
    },

    #[error("insufficient points: need at least {required}, got {provided}")]
    InsufficientPoints {
        required: usize,
        provided: usize,
    },

    #[error("wrong number of parameters: expected {expected}, got {provided}")]
    WrongParameterCount {
        expected: usize,
        provided: usize,
    },

    #[error("day counter generation failed: {0}")]
    DayCounterGeneration(String),
}