use std::sync::Arc;

use chrono::NaiveDate;
use serde::{
    Deserialize,
    Serialize
};

use crate::math::curve::curve::{
    Curve,
    CurveIntegration
};
use crate::math::curve::nonparametriccurve::nonparametriccurve::{NonparametricCurve, Point2D};
use crate::math::curve::nonparametriccurve::piecewisepolynomial::{PiecewisePolynomial, PolynomialType};
use crate::model::interestrate::deterministicinterestratecurve::curvegenerationerror::CurveGenerationError;
use crate::model::interestrate::deterministicinterestratecurve::deterministicinterestratecurve::{
    DeterministicInterestRateCurve, DeterministicInterestRateCurveGenerator, InstantaneousForwardRateCurve
};
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::time::daycounter::daycounter::DayCounter;


/// curve插值的目標變數：
/// - LogDiscount:                curve存ln(P(t))，discount = exp(v)，inst_forward = -v'
/// - ZeroRate:                   curve存r(t)，    discount = exp(-r*t)，inst_forward = r + t*r'
/// - InstantaneousForwardRate:   curve存f(t)，    discount = exp(-∫f)，inst_forward = f
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpolationTarget {
    LogDiscount,
    ZeroRate,
    InstantaneousForwardRate
}


/// 單側外插方式，左右側可獨立設定
/// FlatForwardRate：在邊界節點的inst_forward值維持不變
///   左側：discount(t) = exp(-f_left * t)
///   右側：discount(t) = discount(max_t) * exp(-f_right * (t - max_t))
/// 注意：LogDiscount與ZeroRate搭配FlatForwardRate時需要curve預計算導數
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrapolationMethod {
    Default,
    FlatForwardRate,
}

pub struct PiecewisePolyInterestRateCurve {
    reference_date: NaiveDate,
    day_counter: Arc<DayCounter>,
    curve: PiecewisePolynomial,
    interpolation_target: InterpolationTarget,
    left_extrapolation: ExtrapolationMethod,
    right_extrapolation: ExtrapolationMethod,
    // 邊界預算值，建構時計算一次：
    // left_anchor_inst_forward：min_t處的inst_forward，左側flat forward外插用
    // right_anchor_inst_forward：max_t處的inst_forward，右側flat forward外插用
    // right_anchor_discount：max_t處的discount，右側flat forward外插的起始點
    min_t: f64,
    max_t: f64,
    left_anchor_inst_forward: f64,
    right_anchor_inst_forward: f64,
    right_anchor_discount: f64,
}

impl PiecewisePolyInterestRateCurve {
    pub fn new(
        reference_date: NaiveDate,
        day_counter: Arc<DayCounter>,
        curve: PiecewisePolynomial,
        interpolation_target: InterpolationTarget,
        left_extrapolation: ExtrapolationMethod,
        right_extrapolation: ExtrapolationMethod,
    ) -> Self {
        let min_t = curve.min_x();
        let max_t = curve.max_x();

        let left_anchor_inst_forward =
            Self::compute_inst_forward_at(&curve, &interpolation_target, min_t);
        let right_anchor_inst_forward =
            Self::compute_inst_forward_at(&curve, &interpolation_target, max_t);
        let right_anchor_discount =
            Self::compute_discount_at(&curve, &interpolation_target, max_t);

        Self {
            reference_date,
            day_counter,
            curve,
            interpolation_target,
            left_extrapolation,
            right_extrapolation,
            min_t,
            max_t,
            left_anchor_inst_forward,
            right_anchor_inst_forward,
            right_anchor_discount,
        }
    }

    /// 計算指定t處的inst_forward
    /// - LogDiscount:              f = -v'(t)
    /// - ZeroRate:                 f = r(t) + t * r'(t)
    /// - InstantaneousForwardRate: f = v(t)
    fn compute_inst_forward_at(
        curve: &PiecewisePolynomial,
        interpolation_target: &InterpolationTarget,
        t: f64,
    ) -> f64 {
        match interpolation_target {
            InterpolationTarget::LogDiscount => {
                -curve.derivative(t)
            }
            InterpolationTarget::ZeroRate => {
                curve.value(t) + t * curve.derivative(t)
            }
            InterpolationTarget::InstantaneousForwardRate => {
                curve.value(t)
            }
        }
    }

    /// 計算指定t處的discount
    /// - LogDiscount:              P = exp(v(t))
    /// - ZeroRate:                 P = exp(-r(t) * t)
    /// - InstantaneousForwardRate: P = exp(-∫f dt)
    fn compute_discount_at(
        curve: &PiecewisePolynomial,
        interpolation_target: &InterpolationTarget,
        t: f64,
    ) -> f64 {
        match interpolation_target {
            InterpolationTarget::LogDiscount => curve.value(t).exp(),
            InterpolationTarget::ZeroRate => (-curve.value(t) * t).exp(),
            InterpolationTarget::InstantaneousForwardRate => (-curve.integral(0.0, t)).exp(),
        }
    }

    pub fn interpolation_target(&self) -> &InterpolationTarget {
        &self.interpolation_target
    }

    pub fn left_extrapolation(&self) -> ExtrapolationMethod {
        self.left_extrapolation
    }

    pub fn right_extrapolation(&self) -> ExtrapolationMethod {
        self.right_extrapolation
    }
}

impl InterestRateCurve for PiecewisePolyInterestRateCurve {
    fn day_counter(&self) -> Arc<DayCounter> {
        self.day_counter.clone()
    }

    fn reference_date(&self) -> NaiveDate {
        self.reference_date
    }

    fn discount(&self, d: NaiveDate) -> f64 {
        // reference_date當天discount定義為1.0，避免t=0時的數值問題
        if d == self.reference_date {
            return 1.0;
        }

        let t = self.year_fraction(d);

        if t < self.min_t {
            return match self.left_extrapolation {
                ExtrapolationMethod::FlatForwardRate => {
                    (-self.left_anchor_inst_forward * t).exp()
                }
                ExtrapolationMethod::Default => {
                    Self::compute_discount_at(&self.curve, &self.interpolation_target, t)
                }
            };
        }

        if t > self.max_t {
            return match self.right_extrapolation {
                ExtrapolationMethod::FlatForwardRate => {
                    self.right_anchor_discount
                        * (-self.right_anchor_inst_forward * (t - self.max_t)).exp()
                }
                ExtrapolationMethod::Default => {
                    Self::compute_discount_at(&self.curve, &self.interpolation_target, t)
                }
            };
        }

        Self::compute_discount_at(&self.curve, &self.interpolation_target, t)
    }
}

impl InstantaneousForwardRateCurve for PiecewisePolyInterestRateCurve {
    fn inst_forward(&self, date: NaiveDate) -> f64 {
        let t = self.year_fraction(date);

        if t < self.min_t {
            return match self.left_extrapolation {
                ExtrapolationMethod::FlatForwardRate => self.left_anchor_inst_forward,
                ExtrapolationMethod::Default => {
                    Self::compute_inst_forward_at(&self.curve, &self.interpolation_target, t)
                }
            };
        }

        if t > self.max_t {
            return match self.right_extrapolation {
                ExtrapolationMethod::FlatForwardRate => self.right_anchor_inst_forward,
                ExtrapolationMethod::Default => {
                    Self::compute_inst_forward_at(&self.curve, &self.interpolation_target, t)
                }
            };
        }

        Self::compute_inst_forward_at(&self.curve, &self.interpolation_target, t)
    }
}

impl DeterministicInterestRateCurve for PiecewisePolyInterestRateCurve {}


pub struct PiecewisePolyInterestRateCurveGenerator {
    reference_date: NaiveDate,
    day_counter: Arc<DayCounter>,
    polynomial_type: PolynomialType,
    interpolation_target: InterpolationTarget,
    left_extrapolation: ExtrapolationMethod,
    right_extrapolation: ExtrapolationMethod,
    dates: Vec<NaiveDate>,
    times: Vec<f64>,
}

impl PiecewisePolyInterestRateCurveGenerator {
    pub fn new(
        reference_date: NaiveDate,
        day_counter: Arc<DayCounter>,
        polynomial_type: PolynomialType,
        interpolation_target: InterpolationTarget,
        left_extrapolation: ExtrapolationMethod,
        right_extrapolation: ExtrapolationMethod,
    ) -> Self {
        Self {
            reference_date,
            day_counter,
            polynomial_type,
            interpolation_target,
            left_extrapolation,
            right_extrapolation,
            dates: Vec::new(),
            times: Vec::new(),
        }
    }

    pub fn polynomial_type(&self) -> PolynomialType {
        self.polynomial_type
    }

    pub fn interpolation_target(&self) -> InterpolationTarget {
        self.interpolation_target
    }

    pub fn left_extrapolation(&self) -> ExtrapolationMethod {
        self.left_extrapolation
    }

    pub fn right_extrapolation(&self) -> ExtrapolationMethod {
        self.right_extrapolation
    }

    pub fn dates(&self) -> &Vec<NaiveDate> {
        &self.dates
    }

    pub fn set_dates(&mut self, dates: Vec<NaiveDate>) {
        self.dates = dates;
        self.times = self
            .dates
            .iter()
            .map(|d| self.day_counter.year_fraction(self.reference_date, *d))
            .collect(); 
    }
}


impl DeterministicInterestRateCurveGenerator for PiecewisePolyInterestRateCurveGenerator {
    fn reference_date(&self) -> NaiveDate {
        self.reference_date
    }

    fn day_counter(&self) -> &Arc<DayCounter> {
        &self.day_counter
    }

    fn generate(
        &self,
        values: Vec<f64>,
    ) -> Result<Arc<dyn DeterministicInterestRateCurve>, CurveGenerationError> {
        // values長度必須與dates一致
        if values.len() != self.times.len() {
            return Err(CurveGenerationError::LengthMismatch {
                values_len: values.len(),
                dates_len: self.times.len(),
            });
        }

        let points: Vec<Point2D> = self
            .times
            .iter()
            .cloned()
            .zip(values.iter().cloned())
            .map(|(t, v)| Point2D::new(t, v))
            .collect();

        let curve = match self.interpolation_target {
            InterpolationTarget::InstantaneousForwardRate => {
                PiecewisePolynomial::new_with_integrals(self.polynomial_type, points)
            }
            InterpolationTarget::LogDiscount | InterpolationTarget::ZeroRate => {
                PiecewisePolynomial::new_with_derivatives(self.polynomial_type, points)
            }
        };

        // new_*回傳None代表節點數量不足以建構指定的polynomial type
        let curve = curve.ok_or(CurveGenerationError::InsufficientPoints {
            // 實際提供的節點數
            provided: self.times.len(),
            // polynomial type的最低需求由PiecewisePolynomial內部決定，
            // 此處以2作為已知下限（所有type至少需要2個點），
            // 實際錯誤訊息已足以讓呼叫端理解問題
            required: 2,
        })?;

        Ok(Arc::new(PiecewisePolyInterestRateCurve::new(
            self.reference_date,
            self.day_counter.clone(),
            curve,
            self.interpolation_target,
            self.left_extrapolation,
            self.right_extrapolation,
        )))
    }
}