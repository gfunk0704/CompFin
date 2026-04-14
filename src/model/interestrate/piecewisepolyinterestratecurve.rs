use std::sync::Arc;
use std::sync::OnceLock;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::math::curve::curve::{Curve, CurveIntegral, DerivativeCurve, ValueCurve};
use crate::math::curve::nonparametriccurve::nonparametriccurve::{NonparametricCurve, Point2D};
use crate::math::curve::nonparametriccurve::piecewisepolynomial::{
    PiecewisePolynomial, PolynomialType,
};
use crate::model::interestrate::curvegenerationerror::CurveGenerationError;
use crate::model::interestrate::interestratecurve::{
    DiscountCurve, InstForwardCurve, InterestRateCurve, InterestRateCurveGenerator,
    YearFractionCalculator, ZeroRateCurve,
};
use crate::time::daycounter::daycounter::DayCounterGenerator;


// ─────────────────────────────────────────────────────────────────────────────
// InterpolationTarget / ExtrapolationMethod
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpolationTarget {
    LogDiscount,
    ZeroRate,
    InstantaneousForwardRate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrapolationMethod {
    Default,
    FlatForwardRate,
}


// ─────────────────────────────────────────────────────────────────────────────
// PiecewiseCurveInner
// ─────────────────────────────────────────────────────────────────────────────
//
// `value_curve` 永遠需要，建構時建立。
//
// `deriv_curve`：只有 `LogDiscount` / `ZeroRate` 在呼叫 `to_inst_forward_curve()`
//   時才需要，用 `OnceLock` 按需建立。
//
// `integral_curve`：只有 `InstantaneousForwardRate` 在 `discount_inner()` 時需要，
//   同樣用 `OnceLock` 按需建立。
//
// `polynomial` 保留在 inner 中以供 lazy 初始化使用。

struct PiecewiseCurveInner {
    yfc:                       YearFractionCalculator,
    polynomial:                PiecewisePolynomial,
    interpolation_target:      InterpolationTarget,
    left_extrapolation:        ExtrapolationMethod,
    right_extrapolation:       ExtrapolationMethod,
    min_t:                     f64,
    max_t:                     f64,
    left_anchor_inst_forward:  f64,
    right_anchor_inst_forward: f64,
    right_anchor_discount:     f64,
    /// 永遠需要，建構時建立。
    value_curve:               Arc<dyn ValueCurve>,
    /// Lazy：LogDiscount / ZeroRate 的 inst_forward 計算才需要。
    deriv_curve:               OnceLock<Arc<dyn DerivativeCurve>>,
    /// Lazy：InstantaneousForwardRate 的 discount 計算才需要。
    integral_curve:            OnceLock<Arc<dyn CurveIntegral>>,
}

impl PiecewiseCurveInner {
    /// 取得（或初始化）導數 curve。
    fn deriv_curve(&self) -> &Arc<dyn DerivativeCurve> {
        self.deriv_curve.get_or_init(|| self.polynomial.to_derivative_curve())
    }

    /// 取得（或初始化）積分 curve。
    fn integral_curve(&self) -> &Arc<dyn CurveIntegral> {
        self.integral_curve.get_or_init(|| self.polynomial.to_integral_curve())
    }

    fn discount_at(&self, t: f64) -> f64 {
        if t < self.min_t {
            return match self.left_extrapolation {
                ExtrapolationMethod::FlatForwardRate =>
                    (-self.left_anchor_inst_forward * t).exp(),
                ExtrapolationMethod::Default =>
                    self.discount_inner(t),
            };
        }
        if t > self.max_t {
            return match self.right_extrapolation {
                ExtrapolationMethod::FlatForwardRate =>
                    self.right_anchor_discount
                        * (-self.right_anchor_inst_forward * (t - self.max_t)).exp(),
                ExtrapolationMethod::Default =>
                    self.discount_inner(t),
            };
        }
        self.discount_inner(t)
    }

    fn inst_forward_at(&self, t: f64) -> f64 {
        if t < self.min_t {
            return match self.left_extrapolation {
                ExtrapolationMethod::FlatForwardRate => self.left_anchor_inst_forward,
                ExtrapolationMethod::Default         => self.inst_forward_inner(t),
            };
        }
        if t > self.max_t {
            return match self.right_extrapolation {
                ExtrapolationMethod::FlatForwardRate => self.right_anchor_inst_forward,
                ExtrapolationMethod::Default         => self.inst_forward_inner(t),
            };
        }
        self.inst_forward_inner(t)
    }

    fn zero_rate_at(&self, t: f64) -> f64 {
        -self.discount_at(t).ln() / t
    }

    fn discount_inner(&self, t: f64) -> f64 {
        match self.interpolation_target {
            InterpolationTarget::LogDiscount =>
                self.value_curve.value(t).exp(),
            InterpolationTarget::ZeroRate =>
                (-self.value_curve.value(t) * t).exp(),
            InterpolationTarget::InstantaneousForwardRate =>
                (-self.integral_curve().integral(0.0, t)).exp(),
        }
    }

    fn inst_forward_inner(&self, t: f64) -> f64 {
        match self.interpolation_target {
            InterpolationTarget::LogDiscount =>
                -self.deriv_curve().derivative(t),
            InterpolationTarget::ZeroRate => {
                let r  = self.value_curve.value(t);
                let dr = self.deriv_curve().derivative(t);
                r + t * dr
            }
            InterpolationTarget::InstantaneousForwardRate =>
                self.value_curve.value(t),
        }
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// Wrapper structs
// ─────────────────────────────────────────────────────────────────────────────

pub struct PiecewisePolyDiscountCurve(Arc<PiecewiseCurveInner>);
pub struct PiecewisePolyZeroRateCurve(Arc<PiecewiseCurveInner>);
pub struct PiecewisePolyInstForwardCurve(Arc<PiecewiseCurveInner>);

impl DiscountCurve for PiecewisePolyDiscountCurve {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator { &self.0.yfc }
    fn discount(&self, d: NaiveDate) -> f64 {
        if d == self.0.yfc.reference_date() { return 1.0; }
        self.0.discount_at(self.year_fraction(d))
    }
}

impl ZeroRateCurve for PiecewisePolyZeroRateCurve {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator { &self.0.yfc }
    fn zero_rate(&self, d: NaiveDate) -> f64 {
        self.0.zero_rate_at(self.year_fraction(d))
    }
}

impl InstForwardCurve for PiecewisePolyInstForwardCurve {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator { &self.0.yfc }
    fn inst_forward(&self, d: NaiveDate) -> f64 {
        self.0.inst_forward_at(self.year_fraction(d))
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// PiecewisePolyInterestRateCurve
// ─────────────────────────────────────────────────────────────────────────────

pub struct PiecewisePolyInterestRateCurve {
    inner: Arc<PiecewiseCurveInner>,
}

impl PiecewisePolyInterestRateCurve {
    pub fn new(
        yfc:                  YearFractionCalculator,
        polynomial:           PiecewisePolynomial,
        interpolation_target: InterpolationTarget,
        left_extrapolation:   ExtrapolationMethod,
        right_extrapolation:  ExtrapolationMethod,
    ) -> Self {
        let min_t = polynomial.min_x();
        let max_t = polynomial.max_x();

        // value_curve 永遠需要，建構時建立
        let value_curve = polynomial.to_value_curve();

        // 錨點計算：只在 FlatForwardRate 外插時才用到，
        // 用臨時建立的 deriv/integral curve 計算後即丟棄，
        // 不存在 inner 裡（inner 的 deriv/integral 由 OnceLock lazy 建立）
        let left_anchor_inst_forward = Self::compute_inst_forward(
            &interpolation_target, min_t, &value_curve, &polynomial,
        );
        let right_anchor_inst_forward = Self::compute_inst_forward(
            &interpolation_target, max_t, &value_curve, &polynomial,
        );
        let right_anchor_discount = Self::compute_discount(
            &interpolation_target, max_t, &value_curve, &polynomial,
        );

        Self {
            inner: Arc::new(PiecewiseCurveInner {
                yfc,
                polynomial,
                interpolation_target,
                left_extrapolation,
                right_extrapolation,
                min_t,
                max_t,
                left_anchor_inst_forward,
                right_anchor_inst_forward,
                right_anchor_discount,
                value_curve,
                deriv_curve:    OnceLock::new(),
                integral_curve: OnceLock::new(),
            }),
        }
    }

    /// 計算指定 t 處的 discount，用於建構時的錨點計算。
    /// 按需臨時建立所需的 math curve，不儲存。
    fn compute_discount(
        target:      &InterpolationTarget,
        t:           f64,
        value_curve: &Arc<dyn ValueCurve>,
        polynomial:  &PiecewisePolynomial,
    ) -> f64 {
        match target {
            InterpolationTarget::LogDiscount =>
                value_curve.value(t).exp(),
            InterpolationTarget::ZeroRate =>
                (-value_curve.value(t) * t).exp(),
            InterpolationTarget::InstantaneousForwardRate =>
                (-polynomial.to_integral_curve().integral(0.0, t)).exp(),
        }
    }

    /// 計算指定 t 處的 inst_forward，用於建構時的錨點計算。
    fn compute_inst_forward(
        target:      &InterpolationTarget,
        t:           f64,
        value_curve: &Arc<dyn ValueCurve>,
        polynomial:  &PiecewisePolynomial,
    ) -> f64 {
        match target {
            InterpolationTarget::LogDiscount =>
                -polynomial.to_derivative_curve().derivative(t),
            InterpolationTarget::ZeroRate =>
                value_curve.value(t) + t * polynomial.to_derivative_curve().derivative(t),
            InterpolationTarget::InstantaneousForwardRate =>
                value_curve.value(t),
        }
    }

    pub fn interpolation_target(&self) -> InterpolationTarget { self.inner.interpolation_target }
    pub fn left_extrapolation(&self)   -> ExtrapolationMethod  { self.inner.left_extrapolation }
    pub fn right_extrapolation(&self)  -> ExtrapolationMethod  { self.inner.right_extrapolation }
}

impl InterestRateCurve for PiecewisePolyInterestRateCurve {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator { &self.inner.yfc }

    fn to_discount_curve(&self) -> Arc<dyn DiscountCurve> {
        Arc::new(PiecewisePolyDiscountCurve(Arc::clone(&self.inner)))
    }

    fn to_zero_rate_curve(&self) -> Arc<dyn ZeroRateCurve> {
        Arc::new(PiecewisePolyZeroRateCurve(Arc::clone(&self.inner)))
    }

    fn to_inst_forward_curve(&self) -> Arc<dyn InstForwardCurve> {
        Arc::new(PiecewisePolyInstForwardCurve(Arc::clone(&self.inner)))
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// PiecewisePolyInterestRateCurveGenerator
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// PiecewisePolyInterestRateCurveGenerator
// ─────────────────────────────────────────────────────────────────────────────
//
// 靜態設定：插值方式、外插方式、day counter generator、pillar dates。
// reference_date 不存在 generator 裡，在 generate() 時才傳入，
// 讓同一個 generator 可以在不同評價日使用。
//
// `generate()`（trait）：用已設好的 dates + 傳入的 reference_date。
// `generate_with_dates()`：臨時傳入 dates，供 calibrator 直接使用。

pub struct PiecewisePolyInterestRateCurveGenerator {
    day_counter_generator: Arc<DayCounterGenerator>,
    polynomial_type:       PolynomialType,
    interpolation_target:  InterpolationTarget,
    left_extrapolation:    ExtrapolationMethod,
    right_extrapolation:   ExtrapolationMethod,
    dates:                 Vec<NaiveDate>,
}

impl PiecewisePolyInterestRateCurveGenerator {
    pub fn new(
        day_counter_generator: Arc<DayCounterGenerator>,
        polynomial_type:       PolynomialType,
        interpolation_target:  InterpolationTarget,
        left_extrapolation:    ExtrapolationMethod,
        right_extrapolation:   ExtrapolationMethod,
    ) -> Self {
        Self {
            day_counter_generator,
            polynomial_type,
            interpolation_target,
            left_extrapolation,
            right_extrapolation,
            dates: Vec::new(),
        }
    }

    pub fn polynomial_type(&self)      -> PolynomialType      { self.polynomial_type }
    pub fn interpolation_target(&self) -> InterpolationTarget { self.interpolation_target }
    pub fn left_extrapolation(&self)   -> ExtrapolationMethod  { self.left_extrapolation }
    pub fn right_extrapolation(&self)  -> ExtrapolationMethod  { self.right_extrapolation }
    pub fn dates(&self)                -> &[NaiveDate]         { &self.dates }
    pub fn day_counter_generator(&self) -> &Arc<DayCounterGenerator> { &self.day_counter_generator }

    pub fn set_dates(&mut self, dates: Vec<NaiveDate>) {
        self.dates = dates;
    }

    /// 核心邏輯：用指定的 reference_date 和 dates 產生 curve。
    /// `generate()`（trait）和 `generate_with_dates()` 都委派到這裡。
    pub fn generate_with_dates(
        &self,
        reference_date: NaiveDate,
        dates:          &[NaiveDate],
        values:         Vec<f64>,
    ) -> Result<Arc<dyn InterestRateCurve>, CurveGenerationError> {
        if values.len() != dates.len() {
            return Err(CurveGenerationError::LengthMismatch {
                values_len: values.len(),
                dates_len:  dates.len(),
            });
        }

        let day_counter = self.day_counter_generator
            .generate(None)
            .map_err(|e| CurveGenerationError::DayCounterGeneration(e.to_string()))?;

        let yfc = YearFractionCalculator::new(reference_date, Arc::new(day_counter));

        let points: Vec<Point2D> = dates.iter()
            .map(|d| yfc.year_fraction(*d))
            .zip(values.iter().cloned())
            .map(|(t, v)| Point2D::new(t, v))
            .collect();

        let polynomial = PiecewisePolynomial::new(self.polynomial_type, points)
            .ok_or(CurveGenerationError::InsufficientPoints {
                provided: dates.len(),
                required: 2,
            })?;

        Ok(Arc::new(PiecewisePolyInterestRateCurve::new(
            yfc,
            polynomial,
            self.interpolation_target,
            self.left_extrapolation,
            self.right_extrapolation,
        )))
    }
}

impl InterestRateCurveGenerator for PiecewisePolyInterestRateCurveGenerator {
    fn generate(
        &self,
        reference_date: NaiveDate,
        values:         Vec<f64>,
    ) -> Result<Arc<dyn InterestRateCurve>, CurveGenerationError> {
        self.generate_with_dates(reference_date, &self.dates, values)
    }

    fn generate_with_dates(
        &self,
        reference_date: NaiveDate,
        dates:          &[NaiveDate],
        values:         Vec<f64>,
    ) -> Result<Arc<dyn InterestRateCurve>, CurveGenerationError> {
        // 委派到 inherent method（同名），已實作完整的建構邏輯
        PiecewisePolyInterestRateCurveGenerator::generate_with_dates(self, reference_date, dates, values)
    }
}
