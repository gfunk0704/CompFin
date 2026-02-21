// ── compoundingconvention.rs ─────────────────────────────────────────────────
//
// CompoundingRateIndex 和 DailyCompoundedRateCalculator 共用的 convention 型別。

use std::collections::HashMap;
use chrono::NaiveDate;


// ─────────────────────────────────────────────────────────────────────────────
// FixingConvention
// ─────────────────────────────────────────────────────────────────────────────

/// Overnight rate 的 fixing date 相對於 accrual date 的方向。
///
/// - `Advance`：rate at t 適用 [t, t+1)，fixing date = accrual start date
/// - `Arrear` ：rate at t+1 適用 [t, t+1)，fixing date = accrual end date
///
/// `Advance` + `lookback_days == 0` + `lockout_days == 0` 是 Arbitrage-Free 的必要條件。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FixingConvention {
    Advance,
    Arrear,
}


// ─────────────────────────────────────────────────────────────────────────────
// MissingFixingHandler
// ─────────────────────────────────────────────────────────────────────────────

/// 缺少 past fixing 時的處理策略。
///
/// - `Null`：直接 panic（適合生產環境，確保資料完整性）
/// - `PreviousFixing`：使用最近一個可用的 past fixing（適合節假日等正常缺失）
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MissingFixingHandler {
    Null,
    PreviousFixing,
}

pub type MissingFixingFn = fn(&HashMap<NaiveDate, f64>, NaiveDate) -> f64;

pub fn null_missing_fixing(
    fixings: &HashMap<NaiveDate, f64>,
    d: NaiveDate,
) -> f64 {
    *fixings.get(&d).unwrap_or_else(|| panic!("Missing fixing for {d}"))
}

pub fn previous_missing_fixing(
    fixings: &HashMap<NaiveDate, f64>,
    d: NaiveDate,
) -> f64 {
    if let Some(&rate) = fixings.get(&d) {
        return rate;
    }
    let mut dates: Vec<NaiveDate> = fixings.keys().copied().collect();
    dates.sort_unstable();
    let pos = dates.partition_point(|&fd| fd < d);
    assert!(pos > 0, "No fixing available before {d}");
    fixings[&dates[pos - 1]]
}

pub fn missing_fixing_fn_for(handler: MissingFixingHandler) -> MissingFixingFn {
    match handler {
        MissingFixingHandler::Null           => null_missing_fixing,
        MissingFixingHandler::PreviousFixing => previous_missing_fixing,
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// arbitrage_free_applicable
// ─────────────────────────────────────────────────────────────────────────────

/// 判斷 Arbitrage-Free accrual 是否數學上可用。
///
/// 必要條件（三者同時成立）：
///   1. `lookback_days == 0`：fixing_date == accrual_date，telescoping 前提
///   2. `fixing_convention == Advance`：rate at t 適用 [t, t+1)，乘積方向正確
///   3. `lockout_days == 0`：無鎖定，否則期末 N 天的 fixing_date 相同，telescoping 中斷
///
/// # Lockout 如何破壞 telescoping
///
/// ```text
/// business days: [d0, d1, d2, d3, d4]，lockout=2
///
/// 無 lockout：fixings = [f0, f1, f2, f3, f4]
///   ∏ D(f_i)/D(f_{i+1}) = D(f0)/D(f4)  ✓ telescopes
///
/// 有 lockout：fixings = [f0, f1, f2, f2, f2]  ← 最後2天鎖定在 f2
///   D(f0)/D(f1) × D(f1)/D(f2) × D(f2)/D(f2) × D(f2)/D(f2)
///                               ↑ = 1.0        ↑ = 1.0
///   = D(f0)/D(f2)  ≠  D(f0)/D(f4)             ✗ 不成立
/// ```
pub fn arbitrage_free_applicable(
    lookback_days: u32,
    fixing_convention: FixingConvention,
    lockout_days: u32,
) -> bool {
    lookback_days == 0 && fixing_convention == FixingConvention::Advance && lockout_days == 0
}
