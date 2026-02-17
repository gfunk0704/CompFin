

pub trait Curve {
    fn value(&self, x: f64) -> f64;

    fn derivative(&self, x: f64) -> f64;
}

/// Definite integration of a curve.
///
/// Implementations may precompute per-segment antiderivatives at
/// construction time so that each query is O(log n) (segment lookup)
/// rather than requiring numerical integration.
pub trait CurveIntegration {
    /// Returns ∫_a^b f(x) dx.
    ///
    /// Sign convention: `integral(a, b) == -integral(b, a)`.
    /// Values of `a` or `b` outside the curve's domain are clamped to
    /// `[min_x, max_x]`.
    fn integral(&self, a: f64, b: f64) -> f64;
}