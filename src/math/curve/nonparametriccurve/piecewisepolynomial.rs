use ndarray::{Array1, Array2};
use ndarray_linalg::Solve;

use crate::math::curve::curve::{
    Curve, 
    CurveIntegration
};
use crate::math::curve::nonparametriccurve::nonparametriccurve::{
    NonparametricCurve, 
    Point2D
};

// ─────────────────────────────────────────────
// Subpolynomial
// ─────────────────────────────────────────────
//
// 以 Horner 形式儲存多項式係數：coefs = [c_0, c_1, ..., c_n]
// 代表 P(x) = c_0*(x - lhs_x)^n + c_1*(x - lhs_x)^(n-1) + ... + c_n
//
// 可選地預計算：
//   deriv_coefs      — P'(x) 的係數，供快速求導
//   antideriv_coefs  — ∫_{lhs_x}^{x} P(t) dt 的係數，供快速積分

struct Subpolynomial {
    coefs: Vec<f64>,
    deriv_coefs: Option<Vec<f64>>,
    antideriv_coefs: Option<Vec<f64>>,
    lhs_x: f64,
}

impl Subpolynomial {
    pub fn new(
        coefs: Vec<f64>,
        lhs_x: f64,
        with_deriv: bool,
        with_integral: bool,
    ) -> Subpolynomial {
        let deriv_coefs = if with_deriv {
            Some(Self::compute_deriv_coefs(&coefs))
        } else {
            None
        };
        let antideriv_coefs = if with_integral {
            Some(Self::compute_antideriv_coefs(&coefs))
        } else {
            None
        };
        Subpolynomial { coefs, deriv_coefs, antideriv_coefs, lhs_x }
    }

    // P'(x)：各項乘以次數後降階
    // [c_0, ..., c_{n-1}] → [n*c_0, (n-1)*c_1, ..., 1*c_{n-1}]
    fn compute_deriv_coefs(coefs: &[f64]) -> Vec<f64> {
        let n = coefs.len() - 1;
        if n == 0 {
            return vec![0.0];
        }
        (0..n).map(|i| (n - i) as f64 * coefs[i]).collect()
    }

    // ∫_0^u P(t) dt（以 u = x - lhs_x 為變數）
    //
    // coefs[i] 對應次數 (n-i)，積分後次數升為 (n-i+1)，係數除以 (n-i+1)：
    //   結果 = [c_0/(n+1), c_1/n, ..., c_{n-1}/2, c_n, 0.0]
    //                                                     ^^^
    //                                      積分常數 = 0（F(lhs_x) = 0）
    fn compute_antideriv_coefs(coefs: &[f64]) -> Vec<f64> {
        let n = coefs.len() - 1;
        let mut ac = Vec::with_capacity(n + 2);
        for (i, &c) in coefs.iter().enumerate() {
            ac.push(c / (n + 1 - i) as f64);
        }
        ac.push(0.0); // 積分常數
        ac
    }

    pub fn value(&self, x: f64) -> f64 {
        self.evaluate(&self.coefs, x)
    }

    pub fn derivative(&self, x: f64) -> Option<f64> {
        self.deriv_coefs.as_ref().map(|d| self.evaluate(d, x))
    }

    /// ∫_{lhs_x}^{x} P(t) dt — 必須以 with_integral=true 建構。
    pub fn integral_from_lhs(&self, x: f64) -> f64 {
        self.antideriv_coefs
            .as_ref()
            .map(|ac| self.evaluate(ac, x))
            .expect("antiderivative not precomputed: construct with new_with_integrals()")
    }

    // Horner 求值，以 (x - lhs_x) 為自變數
    fn evaluate(&self, coefs: &[f64], x: f64) -> f64 {
        let u = x - self.lhs_x;
        let mut result = coefs[0];
        for &beta in &coefs[1..] {
            result = f64::mul_add(result, u, beta);
        }
        result
    }
}

// ─────────────────────────────────────────────
// Flat / Linear
// ─────────────────────────────────────────────

fn generate_forward_flat_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    points[..(points.len() - 1)].iter().map(|pt| vec![pt.y()]).collect()
}

fn generate_backward_flat_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    points[1..].iter().map(|pt| vec![pt.y()]).collect()
}

fn generate_linear_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    (0..(points.len() - 1))
        .map(|i| vec![
            Point2D::slope(&points[i], &points[i + 1]),
            points[i].y(),
        ])
        .collect()
}

// ─────────────────────────────────────────────
// 共用輔助函數
// ─────────────────────────────────────────────

fn cubic_coefs_from_moments(points: &[Point2D], h: &[f64], m: &[f64]) -> Vec<Vec<f64>> {
    (0..h.len())
        .map(|i| {
            let d = (m[i + 1] - m[i]) / (6.0 * h[i]);
            let c = m[i] / 2.0;
            let b = (points[i + 1].y() - points[i].y()) / h[i]
                  - h[i] * (2.0 * m[i] + m[i + 1]) / 6.0;
            let a = points[i].y();
            vec![d, c, b, a]
        })
        .collect()
}

fn cubic_coefs_from_hermite(points: &[Point2D], h: &[f64], t: &[f64]) -> Vec<Vec<f64>> {
    (0..h.len())
        .map(|i| {
            let dy = points[i + 1].y() - points[i].y();
            let a = points[i].y();
            let b = t[i];
            let c = (3.0 * dy / h[i] - 2.0 * t[i] - t[i + 1]) / h[i];
            let d = (-2.0 * dy / h[i] + t[i] + t[i + 1]) / (h[i] * h[i]);
            vec![d, c, b, a]
        })
        .collect()
}

// ─────────────────────────────────────────────
// CubicSpline（Natural / Financial / Clamped / NotAKnot）
// ─────────────────────────────────────────────

fn build_interior_system(points: &[Point2D], h: &[f64]) -> (Array2<f64>, Array1<f64>) {
    let n = h.len();
    let mut mat = Array2::<f64>::zeros((n + 1, n + 1));
    let mut rhs = Array1::<f64>::zeros(n + 1);
    for i in 1..n {
        mat[[i, i - 1]] = h[i - 1];
        mat[[i, i]]     = 2.0 * (h[i - 1] + h[i]);
        mat[[i, i + 1]] = h[i];
        rhs[i] = 6.0 * (
            (points[i + 1].y() - points[i].y()) / h[i]
          - (points[i].y()     - points[i - 1].y()) / h[i - 1]
        );
    }
    (mat, rhs)
}

fn generate_natural_cubic_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();
    let (mut mat, rhs) = build_interior_system(points, &h);
    mat[[0, 0]] = 1.0;
    mat[[n, n]] = 1.0;
    let m = mat.solve_into(rhs).expect("NaturalCubic: 線性方程組求解失敗");
    cubic_coefs_from_moments(points, &h, m.as_slice().unwrap())
}

fn generate_financial_cubic_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();
    let (mut mat, mut rhs) = build_interior_system(points, &h);
    mat[[0, 0]] = 1.0;
    mat[[n, n - 1]] = 1.0;
    mat[[n, n]]     = 2.0;
    rhs[n] = -6.0 * (points[n].y() - points[n - 1].y()) / (h[n - 1] * h[n - 1]);
    let m = mat.solve_into(rhs).expect("FinancialCubic: 線性方程組求解失敗");
    cubic_coefs_from_moments(points, &h, m.as_slice().unwrap())
}

fn generate_clamped_cubic_coef_list(
    points: &[Point2D],
    deriv_left: f64,
    deriv_right: f64,
) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();
    let (mut mat, mut rhs) = build_interior_system(points, &h);
    mat[[0, 0]] = 2.0 * h[0];
    mat[[0, 1]] = h[0];
    rhs[0] = 6.0 * ((points[1].y() - points[0].y()) / h[0] - deriv_left);
    mat[[n, n - 1]] = h[n - 1];
    mat[[n, n]]     = 2.0 * h[n - 1];
    rhs[n] = 6.0 * (deriv_right - (points[n].y() - points[n - 1].y()) / h[n - 1]);
    let m = mat.solve_into(rhs).expect("ClampedCubic: 線性方程組求解失敗");
    cubic_coefs_from_moments(points, &h, m.as_slice().unwrap())
}

fn generate_not_a_knot_cubic_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();
    let (mut mat, rhs) = build_interior_system(points, &h);
    mat[[0, 0]] =  -h[1];
    mat[[0, 1]] =   h[0] + h[1];
    mat[[0, 2]] =  -h[0];
    mat[[n, n - 2]] = -h[n - 1];
    mat[[n, n - 1]] =  h[n - 2] + h[n - 1];
    mat[[n, n]]     = -h[n - 2];
    let m = mat.solve_into(rhs).expect("NotAKnotCubic: 線性方程組求解失敗");
    cubic_coefs_from_moments(points, &h, m.as_slice().unwrap())
}

// ─────────────────────────────────────────────
// Akima / Modified Akima
// ─────────────────────────────────────────────

fn akima_slopes(points: &[Point2D], h: &[f64], modified: bool) -> Vec<f64> {
    let n = h.len();
    let s: Vec<f64> = (0..n)
        .map(|i| (points[i + 1].y() - points[i].y()) / h[i])
        .collect();
    let s1  = if n > 1 { s[1] } else { s[0] };
    let sn2 = if n > 1 { s[n - 2] } else { s[n - 1] };
    let s_m1 = 2.0 * s[0]     - s1;
    let s_m2 = 2.0 * s_m1     - s[0];
    let s_p1 = 2.0 * s[n - 1] - sn2;
    let s_p2 = 2.0 * s_p1     - s[n - 1];
    let mut ext = Vec::with_capacity(n + 4);
    ext.push(s_m2);
    ext.push(s_m1);
    ext.extend_from_slice(&s);
    ext.push(s_p1);
    ext.push(s_p2);
    (0..=n)
        .map(|i| {
            let sm2 = ext[i];
            let sm1 = ext[i + 1];
            let sp0 = ext[i + 2];
            let sp1 = ext[i + 3];
            let (w1, w2) = if modified {
                (
                    (sp1 - sp0).abs() + (sp1 + sp0).abs() / 2.0,
                    (sm1 - sm2).abs() + (sm1 + sm2).abs() / 2.0,
                )
            } else {
                ((sp1 - sp0).abs(), (sm1 - sm2).abs())
            };
            if w1 + w2 < f64::EPSILON {
                (sm1 + sp0) / 2.0
            } else {
                (w1 * sm1 + w2 * sp0) / (w1 + w2)
            }
        })
        .collect()
}

fn generate_akima_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();
    let t = akima_slopes(points, &h, false);
    cubic_coefs_from_hermite(points, &h, &t)
}

fn generate_modified_akima_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();
    let t = akima_slopes(points, &h, true);
    cubic_coefs_from_hermite(points, &h, &t)
}

// ─────────────────────────────────────────────
// PCHIP
// ─────────────────────────────────────────────

fn generate_pchip_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();
    let s: Vec<f64> = (0..n)
        .map(|i| (points[i + 1].y() - points[i].y()) / h[i])
        .collect();
    let mut t = vec![0.0_f64; n + 1];
    if n == 1 {
        t[0] = s[0];
        t[1] = s[0];
        return cubic_coefs_from_hermite(points, &h, &t);
    }
    for i in 1..n {
        if s[i - 1] * s[i] <= 0.0 {
            t[i] = 0.0;
        } else {
            let w1 = 2.0 * h[i]     + h[i - 1];
            let w2 = h[i] + 2.0 * h[i - 1];
            t[i] = (w1 + w2) / (w1 / s[i - 1] + w2 / s[i]);
        }
    }
    t[0] = {
        let raw = ((2.0 * h[0] + h[1]) * s[0] - h[0] * s[1]) / (h[0] + h[1]);
        if raw.signum() != s[0].signum() {
            0.0
        } else if s[0].signum() != s[1].signum() && raw.abs() > 3.0 * s[0].abs() {
            3.0 * s[0]
        } else {
            raw
        }
    };
    t[n] = {
        let raw = ((2.0 * h[n - 1] + h[n - 2]) * s[n - 1] - h[n - 1] * s[n - 2])
                / (h[n - 1] + h[n - 2]);
        if raw.signum() != s[n - 1].signum() {
            0.0
        } else if s[n - 1].signum() != s[n - 2].signum()
               && raw.abs() > 3.0 * s[n - 1].abs()
        {
            3.0 * s[n - 1]
        } else {
            raw
        }
    };
    cubic_coefs_from_hermite(points, &h, &t)
}

// ─────────────────────────────────────────────
// PolynomialType
// ─────────────────────────────────────────────

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum PolynomialType {
    ForwardFlat,
    BackwardFlat,
    Linear,
    NaturalCubic,
    /// 左端 S''=0（同 Natural），右端 S'=0（水平漸近線，適合利率外插）
    FinancialCubic,
    /// 端點一階導數固定為 0.0（Murex 慣例）
    ClampedCubic,
    /// 需要至少 4 個點（3 個區間）
    NotAKnotCubic,
    AkimaCubic,
    ModifiedAkimaCubic,
    PiecewiseCubicHermite,
}

fn get_necessary_points(polynomial_type: PolynomialType) -> usize {
    match polynomial_type {
        PolynomialType::ForwardFlat           => 1,
        PolynomialType::BackwardFlat          => 1,
        PolynomialType::Linear                => 2,
        PolynomialType::NaturalCubic          => 3,
        PolynomialType::FinancialCubic        => 3,
        PolynomialType::ClampedCubic          => 2,
        PolynomialType::NotAKnotCubic         => 4,
        PolynomialType::AkimaCubic            => 3,
        PolynomialType::ModifiedAkimaCubic    => 3,
        PolynomialType::PiecewiseCubicHermite => 3,
    }
}

// ─────────────────────────────────────────────
// PiecewisePolynomial
// ─────────────────────────────────────────────

pub struct PiecewisePolynomial {
    max_x: f64,
    polynomial_type: PolynomialType,
    subpolynomial_list: Vec<Subpolynomial>,
    has_derivatives: bool,
    /// 從 min_x 到每個 knot 的累積積分。
    /// cumulative_integrals[i] = ∫_{x_0}^{x_i} f(t) dt，長度 = 區段數 + 1。
    /// None 若未以 new_with_integrals 系列建構。
    cumulative_integrals: Option<Vec<f64>>,
}

impl PiecewisePolynomial {
    /// 不預計算導數或積分係數。
    pub fn new(
        polynomial_type: PolynomialType,
        points: Vec<Point2D>,
    ) -> Option<PiecewisePolynomial> {
        Self::new_inner(polynomial_type, points, false, false)
    }

    /// 預計算導數係數（可呼叫 Curve::derivative()）。
    pub fn new_with_derivatives(
        polynomial_type: PolynomialType,
        points: Vec<Point2D>,
    ) -> Option<PiecewisePolynomial> {
        Self::new_inner(polynomial_type, points, true, false)
    }

    /// 預計算積分係數與累積積分（可呼叫 CurveIntegration::integral()）。
    pub fn new_with_integrals(
        polynomial_type: PolynomialType,
        points: Vec<Point2D>,
    ) -> Option<PiecewisePolynomial> {
        Self::new_inner(polynomial_type, points, false, true)
    }

    /// 同時預計算導數與積分係數。
    pub fn new_with_derivatives_and_integrals(
        polynomial_type: PolynomialType,
        points: Vec<Point2D>,
    ) -> Option<PiecewisePolynomial> {
        Self::new_inner(polynomial_type, points, true, true)
    }

    fn new_inner(
        polynomial_type: PolynomialType,
        points: Vec<Point2D>,
        with_deriv: bool,
        with_integrals: bool,
    ) -> Option<PiecewisePolynomial> {
        if points.len() < get_necessary_points(polynomial_type) {
            return None;
        }

        let coef_list = match polynomial_type {
            PolynomialType::ForwardFlat  => generate_forward_flat_coef_list(&points),
            PolynomialType::BackwardFlat => generate_backward_flat_coef_list(&points),
            PolynomialType::Linear       => generate_linear_coef_list(&points),
            PolynomialType::NaturalCubic   => generate_natural_cubic_coef_list(&points),
            PolynomialType::FinancialCubic => generate_financial_cubic_coef_list(&points),
            PolynomialType::ClampedCubic   => {
                generate_clamped_cubic_coef_list(&points, 0.0, 0.0)
            }
            PolynomialType::NotAKnotCubic         => generate_not_a_knot_cubic_coef_list(&points),
            PolynomialType::AkimaCubic            => generate_akima_coef_list(&points),
            PolynomialType::ModifiedAkimaCubic    => generate_modified_akima_coef_list(&points),
            PolynomialType::PiecewiseCubicHermite => generate_pchip_coef_list(&points),
        };

        let n_seg = points.len() - 1;
        let max_x = points.last().unwrap().x();

        let subpolynomial_list: Vec<Subpolynomial> = (0..n_seg)
            .map(|i| Subpolynomial::new(
                coef_list[i].clone(),
                points[i].x(),
                with_deriv,
                with_integrals,
            ))
            .collect();

        // 預計算累積積分：cum[i] = ∫_{x_0}^{x_i} f(t) dt
        let cumulative_integrals = if with_integrals {
            let mut cum = vec![0.0_f64; n_seg + 1];
            for i in 0..n_seg {
                let right_x = if i + 1 < n_seg {
                    subpolynomial_list[i + 1].lhs_x
                } else {
                    max_x
                };
                cum[i + 1] = cum[i] + subpolynomial_list[i].integral_from_lhs(right_x);
            }
            Some(cum)
        } else {
            None
        };

        Some(PiecewisePolynomial {
            subpolynomial_list,
            max_x,
            polynomial_type,
            has_derivatives: with_deriv,
            cumulative_integrals,
        })
    }

    pub fn polynomial_type(&self) -> PolynomialType {
        self.polynomial_type
    }

    pub fn has_derivatives(&self) -> bool {
        self.has_derivatives
    }

    pub fn has_integrals(&self) -> bool {
        self.cumulative_integrals.is_some()
    }

    /// 二分搜尋找出 x 所在區段的索引。
    ///
    /// # Bug 修正
    /// 舊版直接回傳 partition_point(|s| s.lhs_x <= x)，
    /// 但 partition_point 回傳的是第一個 lhs_x > x 的索引，
    /// 正確的區段應為其 - 1，否則每個查詢都會偏移一段。
    fn find_segment(&self, x: f64) -> usize {
        let n = self.subpolynomial_list.len();
        if x <= self.subpolynomial_list[0].lhs_x {
            0
        } else if x >= self.max_x {
            n - 1
        } else {
            // partition_point 回傳第一個 lhs_x > x 的 index p（p >= 1 已確保）
            // x 所在區段為 p - 1
            self.subpolynomial_list
                .partition_point(|s| s.lhs_x <= x)
                - 1
        }
    }

    /// ∫_a^b f(x) dx（已知 a <= b）的內部實作，直接使用預計算結果。
    fn integral_ordered(&self, a: f64, b: f64) -> f64 {
        let cum = self.cumulative_integrals.as_ref().unwrap();

        let i = self.find_segment(a);
        let j = self.find_segment(b);

        if i == j {
            // a 與 b 在同一區段：直接相減
            self.subpolynomial_list[j].integral_from_lhs(b)
                - self.subpolynomial_list[i].integral_from_lhs(a)
        } else {
            // ① 區段 i 的尾段：a → 區段 i 的右邊界
            let right_x_i = self.subpolynomial_list[i + 1].lhs_x;
            let tail_i = self.subpolynomial_list[i].integral_from_lhs(right_x_i)
                       - self.subpolynomial_list[i].integral_from_lhs(a);

            // ② 區段 i+1 到 j-1 的完整積分：O(1) 查表
            let middle = cum[j] - cum[i + 1];

            // ③ 區段 j 的頭段：區段 j 左邊界 → b
            let head_j = self.subpolynomial_list[j].integral_from_lhs(b);

            tail_i + middle + head_j
        }
    }
}

// ─────────────────────────────────────────────
// Trait 實作
// ─────────────────────────────────────────────

impl NonparametricCurve for PiecewisePolynomial {
    fn points(&self) -> Vec<Point2D> {
        let mut pts: Vec<Point2D> = self
            .subpolynomial_list
            .iter()
            .map(|s| Point2D::new(s.lhs_x, s.value(s.lhs_x)))
            .collect();
        pts.push(Point2D::new(
            self.max_x,
            self.subpolynomial_list.last().unwrap().value(self.max_x),
        ));
        pts
    }

    fn min_x(&self) -> f64 {
        self.subpolynomial_list[0].lhs_x
    }

    fn max_x(&self) -> f64 {
        self.max_x
    }
}

impl Curve for PiecewisePolynomial {
    fn value(&self, x: f64) -> f64 {
        let i = self.find_segment(x);
        self.subpolynomial_list[i].value(x)
    }

    fn derivative(&self, x: f64) -> f64 {
        let i = self.find_segment(x);
        self.subpolynomial_list[i]
            .derivative(x)
            .expect("derivative not precomputed: construct with new_with_derivatives()")
    }
}

impl CurveIntegration for PiecewisePolynomial {
    fn integral(&self, a: f64, b: f64) -> f64 {
        assert!(
            self.cumulative_integrals.is_some(),
            "integrals not precomputed: construct with new_with_integrals()"
        );

        // 域外 clamp
        let min_x = self.min_x();
        let a = a.clamp(min_x, self.max_x);
        let b = b.clamp(min_x, self.max_x);

        if a == b {
            return 0.0;
        }

        // 符號慣例：∫_a^b = -∫_b^a
        if a > b {
            -self.integral_ordered(b, a)
        } else {
            self.integral_ordered(a, b)
        }
    }
}
