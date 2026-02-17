use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// Round x to the given number of decimal places, matching Python's round() semantics:
/// - Uses Banker's rounding (round half to even / MidpointNearestEven)
/// - Interprets x according to its actual IEEE 754 binary value, not its decimal literal,
///   which correctly handles cases like round(0.155, 2) == 0.15 (since 0.155 is stored
///   as 0.15499999... in f64).
pub fn round(x: f64, digits: u32) -> f64 {
    let d = Decimal::from_f64_retain(x)
        .unwrap_or_else(|| Decimal::from_f64(x).expect("non-finite f64 passed to round"));
    d.round_dp_with_strategy(digits, RoundingStrategy::MidpointNearestEven)
        .to_f64()
        .unwrap_or(x)
}
