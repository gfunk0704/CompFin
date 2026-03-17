use thiserror::Error;

#[derive(Debug, Error)]
pub enum CurveGenerationError {
    // Piecewise：傳入的values數量與dates數量不符
    #[error("values length {values_len} does not match dates length {dates_len}")]
    LengthMismatch {
        values_len: usize,
        dates_len: usize,
    },

    // Piecewise：節點數量不足以建構指定的polynomial type
    #[error("insufficient points: need at least {required}, got {provided}")]
    InsufficientPoints {
        required: usize,
        provided: usize,
    },

    // Nelson-Siegel等參數化方法：參數數量不符合預期
    #[error("wrong number of parameters: expected {expected}, got {provided}")]
    WrongParameterCount {
        expected: usize,
        provided: usize,
    },
}