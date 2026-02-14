// Cargo.toml 需要加入:
//
// [dependencies]
// ndarray = "0.15"
// ndarray-linalg = { version = "0.16", features = ["openblas-static"] }
//
// 或在 Linux/macOS 上使用 netlib：
// ndarray-linalg = { version = "0.16", features = ["netlib-static"] }

use ndarray::{
    Array1, 
    Array2
};
use ndarray_linalg::Solve;

use crate::math::curve::curve::Curve;
use crate::math::curve::nonparametriccurve::{
    NonparametricCurve, 
    Point2D
};

// ─────────────────────────────────────────────
// Subpolynomial
// ─────────────────────────────────────────────

struct Subpolynomial {
    coefs: Vec<f64>,
    deriv_coefs: Option<Vec<f64>>,
    lhs_x: f64,
}

impl Subpolynomial {
    pub fn new(coefs: Vec<f64>, lhs_x: f64, with_deriv: bool) -> Subpolynomial {
        let deriv_coefs = if with_deriv {
            Some(Self::compute_deriv_coefs(&coefs))
        } else {
            None
        };
        Subpolynomial { coefs, deriv_coefs, lhs_x }
    }

    fn compute_deriv_coefs(coefs: &[f64]) -> Vec<f64> {
        let order = coefs.len() - 1;
        if order == 0 {
            vec![0.0]
        } else {
            (0..order)
                .map(|i| (order - i) as f64 * coefs[i])
                .collect()
        }
    }

    pub fn value(&self, x: f64) -> f64 {
        self.evaluate(&self.coefs, x)
    }

    pub fn derivative(&self, x: f64) -> Option<f64> {
        self.deriv_coefs
            .as_ref()
            .map(|d| self.evaluate(d, x))
    }

    fn evaluate(&self, coefs: &[f64], x: f64) -> f64 {
        let x_diff = x - self.lhs_x;
        let mut result = coefs[0];
        for &beta in &coefs[1..] {
            result = f64::mul_add(result, x_diff, beta);
        }
        result
    }
}

// ─────────────────────────────────────────────
// Flat / Linear（既有）
// ─────────────────────────────────────────────

fn generate_forward_flat_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    points[..(points.len() - 1)]
        .iter()
        .map(|pt| vec![pt.y()])
        .collect()
}

fn generate_backward_flat_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    points[1..]
        .iter()
        .map(|pt| vec![pt.y()])
        .collect()
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

/// 從各節點的二階導數（moments）m[0..=n] 計算各區間的三次多項式係數。
///
/// 每段多項式以 Horner 形式存成 [d, c, b, a]，對應：
///   S_i(x) = a + b*(x-x_i) + c*(x-x_i)^2 + d*(x-x_i)^3
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

/// 從各節點的一階導數（Hermite slopes）t[0..=n] 計算各區間的三次多項式係數。
///
/// 同樣存為 [d, c, b, a]。
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
// CubicSpline（Natural / Clamped / NotAKnot）
// ─────────────────────────────────────────────
//
// 三種邊界條件共用同一個架構：
//   建立 (n+1)×(n+1) 的聯立方程組，求解各節點的二階導數 m[0..=n]，
//   內部方程式由 C² 連續性導出：
//     h[i-1]*m[i-1] + 2*(h[i-1]+h[i])*m[i] + h[i]*m[i+1]
//       = 6*( (y[i+1]-y[i])/h[i] - (y[i]-y[i-1])/h[i-1] )
//   第 0 列與第 n 列依邊界條件設定。

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

/// Natural：端點的二階導數為 0（m[0] = m[n] = 0）
fn generate_natural_cubic_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();

    let (mut mat, rhs) = build_interior_system(points, &h);
    mat[[0, 0]] = 1.0;   // m[0] = 0
    mat[[n, n]] = 1.0;   // m[n] = 0

    let m = mat.solve_into(rhs)
        .expect("NaturalCubic: 線性方程組求解失敗");
    cubic_coefs_from_moments(points, &h, m.as_slice().unwrap())
}

/// Financial：左端二階導數為 0（同 Natural），右端一階導數為 0（水平漸近線）
///
///   左端：m[0] = 0
///   右端：S'(x[n]) = 0，整理後得
///     m[n-1] + 2·m[n] = -6·(y[n]-y[n-1]) / h[n-1]²
fn generate_financial_cubic_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();

    let (mut mat, mut rhs) = build_interior_system(points, &h);

    // 左端：Natural（m[0] = 0）
    mat[[0, 0]] = 1.0;

    // 右端：一階導數為 0
    mat[[n, n - 1]] = 1.0;
    mat[[n, n]]     = 2.0;
    rhs[n] = -6.0 * (points[n].y() - points[n - 1].y()) / (h[n - 1] * h[n - 1]);

    let m = mat.solve_into(rhs)
        .expect("FinancialCubic: 線性方程組求解失敗");
    cubic_coefs_from_moments(points, &h, m.as_slice().unwrap())
}

/// Clamped：端點的一階導數為指定值
///
///   左端：2*h[0]*m[0] + h[0]*m[1]
///           = 6*( (y[1]-y[0])/h[0] - deriv_left )
///   右端：h[n-1]*m[n-1] + 2*h[n-1]*m[n]
///           = 6*( deriv_right - (y[n]-y[n-1])/h[n-1] )
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

    let m = mat.solve_into(rhs)
        .expect("ClampedCubic: 線性方程組求解失敗");
    cubic_coefs_from_moments(points, &h, m.as_slice().unwrap())
}

/// Not-a-knot：第三導數在 x[1] 與 x[n-1] 處連續，
/// 等價於相鄰兩段的三次係數相等：
///
///   在 x[1]：(m[1]-m[0])/h[0] = (m[2]-m[1])/h[1]
///     → -h[1]*m[0] + (h[0]+h[1])*m[1] - h[0]*m[2] = 0
///   在 x[n-1]：(m[n-1]-m[n-2])/h[n-2] = (m[n]-m[n-1])/h[n-1]
///     → -h[n-1]*m[n-2] + (h[n-2]+h[n-1])*m[n-1] - h[n-2]*m[n] = 0
///
/// 至少需要 4 個點（3 個區間）使兩條邊界方程線性獨立。
fn generate_not_a_knot_cubic_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();

    let (mut mat, rhs) = build_interior_system(points, &h);

    // 左端 not-a-knot（利用 x[1]）
    mat[[0, 0]] =  -h[1];
    mat[[0, 1]] =   h[0] + h[1];
    mat[[0, 2]] =  -h[0];

    // 右端 not-a-knot（利用 x[n-1]）
    mat[[n, n - 2]] = -h[n - 1];
    mat[[n, n - 1]] =  h[n - 2] + h[n - 1];
    mat[[n, n]]     = -h[n - 2];

    let m = mat.solve_into(rhs)
        .expect("NotAKnotCubic: 線性方程組求解失敗");
    cubic_coefs_from_moments(points, &h, m.as_slice().unwrap())
}

// ─────────────────────────────────────────────
// Akima / Modified Akima
// ─────────────────────────────────────────────
//
// 不建立聯立方程組；各節點的斜率由鄰近有限差分的加權平均獨立計算。
//
// 標準 Akima 權重：
//   w1 = |s[i+1] - s[i]|
//   w2 = |s[i-1] - s[i-2]|
//
// Modified Akima（makima）權重（改善平坦區域的 overshooting）：
//   w1 = |s[i+1] - s[i]| + |s[i+1] + s[i]| / 2
//   w2 = |s[i-1] - s[i-2]| + |s[i-1] + s[i-2]| / 2
//
// 端點使用外推補齊幽靈點：
//   s[-1] = 2*s[0] - s[1]
//   s[-2] = 2*s[-1] - s[0]
//   s[n]  = 2*s[n-1] - s[n-2]
//   s[n+1]= 2*s[n] - s[n-1]

fn akima_slopes(points: &[Point2D], h: &[f64], modified: bool) -> Vec<f64> {
    let n = h.len();

    // 有限差分 s[i] = (y[i+1]-y[i]) / h[i]，共 n 個
    let s: Vec<f64> = (0..n)
        .map(|i| (points[i + 1].y() - points[i].y()) / h[i])
        .collect();

    // 端點外推幽靈點（n=1 時左右各退化為同一個值）
    let s1 = if n > 1 { s[1] } else { s[0] };
    let sn2 = if n > 1 { s[n - 2] } else { s[n - 1] };

    let s_m1 = 2.0 * s[0]      - s1;
    let s_m2 = 2.0 * s_m1      - s[0];
    let s_np1 = 2.0 * s[n - 1] - sn2;
    let s_np2 = 2.0 * s_np1    - s[n - 1];

    // 擴展陣列：ext[i+2] 對應 s[i]
    let mut ext = Vec::with_capacity(n + 4);
    ext.push(s_m2);
    ext.push(s_m1);
    ext.extend_from_slice(&s);
    ext.push(s_np1);
    ext.push(s_np2);

    // 為每個節點 i（含兩端）計算斜率
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
                (
                    (sp1 - sp0).abs(),
                    (sm1 - sm2).abs(),
                )
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
// PCHIP（Piecewise Cubic Hermite Interpolating Polynomial）
// ─────────────────────────────────────────────
//
// Fritsch-Carlson 方法，保單調性。
//
// 內部節點斜率：調和平均值
//   若 s[i-1]*s[i] <= 0：t[i] = 0（局部極值點）
//   否則：t[i] = (w1+w2) / (w1/s[i-1] + w2/s[i])
//           where w1 = 2*h[i] + h[i-1]，w2 = h[i] + 2*h[i-1]
//
// 端點斜率：單側有限差分修正
//   t = ((2*h[0]+h[1])*s[0] - h[0]*s[1]) / (h[0]+h[1])
//   若符號與 s[0] 相反 → 0
//   若 s[0]*s[1]<=0 且 |t|>3|s[0]| → 3*s[0]

fn generate_pchip_coef_list(points: &[Point2D]) -> Vec<Vec<f64>> {
    let n = points.len() - 1;
    let h: Vec<f64> = (0..n).map(|i| points[i + 1].x() - points[i].x()).collect();
    let s: Vec<f64> = (0..n)
        .map(|i| (points[i + 1].y() - points[i].y()) / h[i])
        .collect();

    let mut t = vec![0.0_f64; n + 1];

    // 只有一個區間：退化為線性
    if n == 1 {
        t[0] = s[0];
        t[1] = s[0];
        return cubic_coefs_from_hermite(points, &h, &t);
    }

    // 內部節點（調和平均）
    for i in 1..n {
        if s[i - 1] * s[i] <= 0.0 {
            t[i] = 0.0;
        } else {
            let w1 = 2.0 * h[i]     + h[i - 1];
            let w2 = h[i] + 2.0 * h[i - 1];
            t[i] = (w1 + w2) / (w1 / s[i - 1] + w2 / s[i]);
        }
    }

    // 左端點（Fritsch-Carlson 端點修正）
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

    // 右端點
    t[n] = {
        let raw = ((2.0 * h[n - 1] + h[n - 2]) * s[n - 1] - h[n - 1] * s[n - 2])
                / (h[n - 1] + h[n - 2]);
        if raw.signum() != s[n - 1].signum() {
            0.0
        } else if s[n - 1].signum() != s[n - 2].signum() && raw.abs() > 3.0 * s[n - 1].abs() {
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
        PolynomialType::NaturalCubic          => 3, // 2點退化為線性
        PolynomialType::FinancialCubic        => 3, // 同 Natural
        PolynomialType::ClampedCubic          => 2,
        PolynomialType::NotAKnotCubic         => 4, // 兩條邊界方程需線性獨立
        PolynomialType::AkimaCubic            => 3, // 2點退化為線性
        PolynomialType::ModifiedAkimaCubic    => 3, // 同上
        PolynomialType::PiecewiseCubicHermite => 3, // 同上
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
}

impl PiecewisePolynomial {
    /// 不預計算導數係數
    pub fn new(
        polynomial_type: PolynomialType,
        points: Vec<Point2D>,
    ) -> Option<PiecewisePolynomial> {
        Self::new_inner(polynomial_type, points, false)
    }

    /// 預計算導數係數（可呼叫 `derivative()`）
    pub fn new_with_derivatives(
        polynomial_type: PolynomialType,
        points: Vec<Point2D>,
    ) -> Option<PiecewisePolynomial> {
        Self::new_inner(polynomial_type, points, true)
    }

    fn new_inner(
        polynomial_type: PolynomialType,
        points: Vec<Point2D>,
        with_deriv: bool,
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

        let subpolynomial_list = (0..(points.len() - 1))
            .map(|i| Subpolynomial::new(coef_list[i].clone(), points[i].x(), with_deriv))
            .collect();

        Some(PiecewisePolynomial {
            subpolynomial_list,
            max_x: points.last().unwrap().x(),
            polynomial_type,
            has_derivatives: with_deriv,
        })
    }

    pub fn polynomial_type(&self) -> PolynomialType {
        self.polynomial_type
    }

    pub fn has_derivatives(&self) -> bool {
        self.has_derivatives
    }

    fn find_segment(&self, x: f64) -> usize {
        if x <= self.min_x() {
            0
        } else if x >= self.max_x {
            self.subpolynomial_list.len() - 1
        } else {
            self.subpolynomial_list
                .partition_point(|s| s.lhs_x <= x)
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
            .expect("Derivative not available: use new_with_derivatives() to construct")
    }
}

