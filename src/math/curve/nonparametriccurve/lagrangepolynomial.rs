use std::sync::Arc;

use crate::math::curve::curve::{
    Curve,
    CurveIntegral,
    DerivativeCurve,
    ValueCurve,
};
use crate::math::curve::nonparametriccurve::nonparametriccurve::{
    NonparametricCurve,
    Point2D,
};


// ─────────────────────────────────────────────────────────────────────────────
// LagrangePolynomial - Barycentric Form (2nd kind)
// ─────────────────────────────────────────────────────────────────────────────
//
// 實作基於 Barycentric 第二類公式（數值穩定性最佳）：
//
//   L(x) = Σ w_i·y_i/(x-x_i) / Σ w_i/(x-x_i)
//
// 其中 barycentric weights:
//   w_i = 1 / Π_{j≠i} (x_i - x_j)
//
// 注意：
//   - Lagrange polynomial 是全域插值，n > 10 可能出現 Runge 振盪
//   - 金融應用中通常使用 PiecewisePolynomial 而非 Lagrange
//   - Lagrange 主要用於：低階外推（2-4點）、特殊場景插值

#[derive(Clone)]
pub struct LagrangePolynomial {
    x_data:  Vec<f64>,
    y_data:  Vec<f64>,
    /// Barycentric weights: w_i = 1 / Π_{j≠i} (x_i - x_j)
    weights: Vec<f64>,
    /// Monomial form 係數 [a_0, a_1, ..., a_n]，代表 Σ a_k·x^k。
    /// None 表示尚未建立積分版本，由 `to_integral_curve()` 按需計算。
    monomial_coefs: Option<Vec<f64>>,
}

impl LagrangePolynomial {
    /// 建立 LagrangePolynomial，不預計算導數或積分係數。
    ///
    /// 需要導數時呼叫 `to_derivative_curve()`，
    /// 需要積分時呼叫 `to_integral_curve()`，
    /// 兩者都會按需建立帶對應係數的版本。
    pub fn new(mut points: Vec<Point2D>) -> Option<Self> {
        if points.is_empty() { return None; }

        // 按 x 排序
        points.sort_by(|a, b| a.x().partial_cmp(&b.x()).unwrap());

        let x_data: Vec<f64> = points.iter().map(|p| p.x()).collect();
        let y_data: Vec<f64> = points.iter().map(|p| p.y()).collect();
        let weights = Self::compute_barycentric_weights(&x_data);

        Some(Self { x_data, y_data, weights, monomial_coefs: None })
    }

    /// 建立帶 monomial 係數的版本（供 `to_integral_curve` 使用）。
    fn with_monomial_coefs(mut self) -> Self {
        self.monomial_coefs = Some(
            Self::convert_to_monomial(&self.x_data, &self.y_data, &self.weights)
        );
        self
    }

    /// 計算 barycentric weights: w_i = 1 / Π_{j≠i} (x_i - x_j)，O(n²)。
    fn compute_barycentric_weights(x_data: &[f64]) -> Vec<f64> {
        let n = x_data.len();
        let mut weights = vec![1.0; n];
        for i in 0..n {
            for j in 0..n {
                if i != j { weights[i] /= x_data[i] - x_data[j]; }
            }
        }
        weights
    }

    /// Lagrange → Newton divided differences → Monomial form。
    ///
    /// 回傳 [a_0, a_1, ..., a_{n-1}]，代表 a_0 + a_1·x + ... + a_{n-1}·x^{n-1}。
    fn convert_to_monomial(x_data: &[f64], y_data: &[f64], _weights: &[f64]) -> Vec<f64> {
        let n = x_data.len();

        // Step 1：divided differences（Newton form 係數）
        let mut f = y_data.to_vec();
        for j in 1..n {
            for i in (j..n).rev() {
                f[i] = (f[i] - f[i - 1]) / (x_data[i] - x_data[i - j]);
            }
        }

        // Step 2：Newton form → Monomial form
        let mut monomial = vec![0.0; n];
        monomial[n - 1] = f[n - 1];
        for i in (0..n - 1).rev() {
            for k in (1..n).rev() {
                monomial[k] = monomial[k - 1] - x_data[i] * monomial[k];
            }
            monomial[0] = -x_data[i] * monomial[0];
            monomial[0] += f[i];
        }
        monomial
    }

    /// Barycentric 第二類求值，特殊處理節點上的 0/0。
    fn value_barycentric(&self, x: f64) -> f64 {
        for (i, &xi) in self.x_data.iter().enumerate() {
            if (x - xi).abs() < f64::EPSILON { return self.y_data[i]; }
        }
        let mut num = 0.0;
        let mut den = 0.0;
        for (i, &xi) in self.x_data.iter().enumerate() {
            let t = self.weights[i] / (x - xi);
            num += t * self.y_data[i];
            den += t;
        }
        num / den
    }

    /// 導數：L'(x) = (N'·D - N·D') / D²。
    ///
    /// 節點上用中心差分近似（避免 0/0）。
    fn derivative_barycentric(&self, x: f64) -> f64 {
        for &xi in &self.x_data {
            if (x - xi).abs() < f64::EPSILON {
                let h = 1e-8;
                return (self.value_barycentric(x + h) - self.value_barycentric(x - h))
                    / (2.0 * h);
            }
        }
        let mut n_val   = 0.0;
        let mut d_val   = 0.0;
        let mut n_prime = 0.0;
        let mut d_prime = 0.0;
        for (i, &xi) in self.x_data.iter().enumerate() {
            let diff = x - xi;
            let t    = self.weights[i] / diff;
            let t_sq = t / diff;
            n_val   +=  t    * self.y_data[i];
            d_val   +=  t;
            n_prime -= t_sq  * self.y_data[i];
            d_prime -= t_sq;
        }
        (n_prime * d_val - n_val * d_prime) / (d_val * d_val)
    }

    /// Monomial form 定積分：∫_a^b Σ c_k·x^k dx。
    fn integral_monomial(&self, a: f64, b: f64) -> f64 {
        let coefs = self.monomial_coefs.as_ref().unwrap();
        coefs.iter().enumerate().map(|(k, &c)| {
            let p = (k + 1) as i32;
            c * (b.powi(p) - a.powi(p)) / (k + 1) as f64
        }).sum()
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// Wrapper structs
// ─────────────────────────────────────────────────────────────────────────────

pub struct LagrangePolynomialValueCurve(LagrangePolynomial);
pub struct LagrangePolynomialDerivativeCurve(LagrangePolynomial);
pub struct LagrangePolynomialIntegralCurve(LagrangePolynomial);

impl ValueCurve for LagrangePolynomialValueCurve {
    fn value(&self, x: f64) -> f64 {
        self.0.value_barycentric(x)
    }
}

impl DerivativeCurve for LagrangePolynomialDerivativeCurve {
    fn derivative(&self, x: f64) -> f64 {
        self.0.derivative_barycentric(x)
    }
}

impl CurveIntegral for LagrangePolynomialIntegralCurve {
    fn integral(&self, a: f64, b: f64) -> f64 {
        if (a - b).abs() < f64::EPSILON { return 0.0; }
        if a > b { -self.0.integral_monomial(b, a) }
        else     {  self.0.integral_monomial(a, b) }
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// Trait 實作
// ─────────────────────────────────────────────────────────────────────────────

impl NonparametricCurve for LagrangePolynomial {
    fn points(&self) -> Vec<Point2D> {
        self.x_data.iter().zip(self.y_data.iter())
            .map(|(&x, &y)| Point2D::new(x, y))
            .collect()
    }

    fn min_x(&self) -> f64 { *self.x_data.first().unwrap() }
    fn max_x(&self) -> f64 { *self.x_data.last().unwrap() }
}

impl Curve for LagrangePolynomial {
    /// 直接 clone，不需要額外計算。
    fn to_value_curve(&self) -> Arc<dyn ValueCurve> {
        Arc::new(LagrangePolynomialValueCurve(self.clone()))
    }

    /// 導數直接從 barycentric 公式算，不需要預計算係數，直接 clone。
    fn to_derivative_curve(&self) -> Arc<dyn DerivativeCurve> {
        Arc::new(LagrangePolynomialDerivativeCurve(self.clone()))
    }

    /// 按需計算 monomial 係數。
    fn to_integral_curve(&self) -> Arc<dyn CurveIntegral> {
        Arc::new(LagrangePolynomialIntegralCurve(self.clone().with_monomial_coefs()))
    }
}