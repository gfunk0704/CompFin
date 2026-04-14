use std::sync::Arc;


// ─────────────────────────────────────────────────────────────────────────────
// 三種計算型態
// ─────────────────────────────────────────────────────────────────────────────

/// 插值求值，只專注於 f(x)。
pub trait ValueCurve: Send + Sync {
    fn value(&self, x: f64) -> f64;
}

/// 一階導數，只專注於 f'(x)。
pub trait DerivativeCurve: Send + Sync {
    fn derivative(&self, x: f64) -> f64;
}

/// 定積分，只專注於 ∫ₐᵇ f(x) dx。
///
/// 實作可在建構時預計算各 segment 的反導數，
/// 讓每次查詢是 O(log n)（segment 搜尋）而不需要數值積分。
pub trait CurveIntegral: Send + Sync {
    /// 回傳 ∫ₐᵇ f(x) dx。
    ///
    /// 符號慣例：`integral(a, b) == -integral(b, a)`。
    /// `a` 或 `b` 超出 curve 範圍時 clamp 到 `[min_x, max_x]`。
    fn integral(&self, a: f64, b: f64) -> f64;
}


// ─────────────────────────────────────────────────────────────────────────────
// Curve
// ─────────────────────────────────────────────────────────────────────────────

pub trait Curve {
    fn to_value_curve(&self)      -> Arc<dyn ValueCurve>;
    fn to_derivative_curve(&self) -> Arc<dyn DerivativeCurve>;
    fn to_integral_curve(&self)   -> Arc<dyn CurveIntegral>;
}
