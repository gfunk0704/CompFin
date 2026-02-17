use crate::math::curve::curve::{
    Curve, 
    CurveIntegration
};
use crate::math::curve::nonparametriccurve::{
    NonparametricCurve, 
    Point2D
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
// 優點：
//   - O(n²) 預計算 weights，O(n) 求值
//   - 極佳的數值穩定性（分子分母同步縮放）
//   - 任意節點順序、分布
//   - 修改 y_i 無需重算 weights
//
// 注意：
//   - Lagrange polynomial 是全域插值，n > 10 可能出現 Runge 振盪
//   - 金融應用中通常使用 PiecewisePolynomial 而非 Lagrange
//   - Lagrange 主要用於：低階外推（2-4點）、特殊場景插值

pub struct LagrangePolynomial {
    x_data: Vec<f64>,
    y_data: Vec<f64>,
    /// Barycentric weights: w_i = 1 / Π_{j≠i} (x_i - x_j)
    weights: Vec<f64>,
    has_derivatives: bool,
    /// Monomial form 係數 [a_0, a_1, ..., a_n]，代表 Σ a_k·x^k
    /// 用於積分計算，None 若未以 new_with_integrals 建構
    monomial_coefs: Option<Vec<f64>>,
}

impl LagrangePolynomial {
    /// 基本建構：僅支援 value() 求值
    pub fn new(mut points: Vec<Point2D>) -> Option<LagrangePolynomial> {
        Self::new_inner(points, false, false)
    }

    /// 預計算導數：支援 derivative()
    pub fn new_with_derivatives(points: Vec<Point2D>) -> Option<LagrangePolynomial> {
        Self::new_inner(points, true, false)
    }

    /// 預計算積分：支援 CurveIntegration::integral()
    ///
    /// 內部將 Lagrange 轉為 monomial form 以便解析積分
    pub fn new_with_integrals(points: Vec<Point2D>) -> Option<LagrangePolynomial> {
        Self::new_inner(points, false, true)
    }

    /// 同時支援導數與積分
    pub fn new_with_derivatives_and_integrals(points: Vec<Point2D>) -> Option<LagrangePolynomial> {
        Self::new_inner(points, true, true)
    }

    fn new_inner(
        mut points: Vec<Point2D>,
        with_deriv: bool,
        with_integrals: bool,
    ) -> Option<LagrangePolynomial> {
        let n = points.len();
        if n == 0 {
            return None;
        }

        // 按 x 排序（為了 NonparametricCurve::min_x/max_x 語義清晰）
        points.sort_by(|a, b| a.x().partial_cmp(&b.x()).unwrap());

        let x_data: Vec<f64> = points.iter().map(|p| p.x()).collect();
        let y_data: Vec<f64> = points.iter().map(|p| p.y()).collect();

        // 預計算 barycentric weights
        let weights = Self::compute_barycentric_weights(&x_data);

        // 預計算 monomial 係數（用於積分）
        let monomial_coefs = if with_integrals {
            Some(Self::convert_to_monomial(&x_data, &y_data, &weights))
        } else {
            None
        };

        Some(LagrangePolynomial {
            x_data,
            y_data,
            weights,
            has_derivatives: with_deriv,
            monomial_coefs,
        })
    }

    /// 計算 barycentric weights: w_i = 1 / Π_{j≠i} (x_i - x_j)
    ///
    /// 複雜度：O(n²)
    fn compute_barycentric_weights(x_data: &[f64]) -> Vec<f64> {
        let n = x_data.len();
        let mut weights = vec![1.0; n];

        for i in 0..n {
            for j in 0..n {
                if i != j {
                    weights[i] /= x_data[i] - x_data[j];
                }
            }
        }

        weights
    }

    /// 將 Lagrange polynomial 轉為 monomial form（power basis）
    ///
    /// 使用 Newton divided differences 作為中介：
    ///   1. Lagrange → Newton form（divided differences）
    ///   2. Newton form → Monomial form（展開）
    ///
    /// 回傳 [a_0, a_1, ..., a_{n-1}]，代表 a_0 + a_1·x + ... + a_{n-1}·x^{n-1}
    fn convert_to_monomial(x_data: &[f64], y_data: &[f64], _weights: &[f64]) -> Vec<f64> {
        let n = x_data.len();

        // Step 1: 計算 divided differences（Newton form 係數）
        let mut f = y_data.to_vec();
        for j in 1..n {
            for i in (j..n).rev() {
                f[i] = (f[i] - f[i - 1]) / (x_data[i] - x_data[i - j]);
            }
        }
        // f[i] 現在是 f[x_0, x_1, ..., x_i]

        // Step 2: Newton form → Monomial form
        // Newton: f[x_0] + f[x_0,x_1]·(x-x_0) + f[x_0,x_1,x_2]·(x-x_0)(x-x_1) + ...
        //
        // 用 Horner-like 展開：從高次往低次累積
        let mut monomial = vec![0.0; n];
        monomial[n - 1] = f[n - 1];

        for i in (0..n - 1).rev() {
            // 當前 monomial[] 代表 (x-x_{i+1})(x-x_{i+2})...(x-x_{n-1})
            // 乘以 (x - x_i) 並加上 f[x_0,...,x_i]

            // 先乘 (x - x_i) = x - x_i
            for k in (1..n).rev() {
                monomial[k] = monomial[k - 1] - x_data[i] * monomial[k];
            }
            monomial[0] = -x_data[i] * monomial[0];

            // 加上 f[x_0,...,x_i]
            monomial[0] += f[i];
        }

        monomial
    }

    pub fn has_derivatives(&self) -> bool {
        self.has_derivatives
    }

    pub fn has_integrals(&self) -> bool {
        self.monomial_coefs.is_some()
    }

    /// 計算多項式在 x 處的值（Barycentric 第二類公式）
    ///
    /// 特殊處理：若 x 恰好是節點，直接回傳 y_i（避免 0/0）
    fn value_barycentric(&self, x: f64) -> f64 {
        let n = self.x_data.len();

        // 檢查是否恰好在節點上（避免除以零）
        for i in 0..n {
            if (x - self.x_data[i]).abs() < f64::EPSILON {
                return self.y_data[i];
            }
        }

        // Barycentric 第二類：L(x) = Σ w_i·y_i/(x-x_i) / Σ w_i/(x-x_i)
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for i in 0..n {
            let temp = self.weights[i] / (x - self.x_data[i]);
            numerator += temp * self.y_data[i];
            denominator += temp;
        }

        numerator / denominator
    }

    /// 計算導數：L'(x) = (N'(x)·D(x) - N(x)·D'(x)) / D(x)²
    ///
    /// 其中：
    ///   N(x)  = Σ w_i·y_i/(x-x_i)
    ///   D(x)  = Σ w_i/(x-x_i)
    ///   N'(x) = -Σ w_i·y_i/(x-x_i)²
    ///   D'(x) = -Σ w_i/(x-x_i)²
    fn derivative_barycentric(&self, x: f64) -> f64 {
        let n = self.x_data.len();

        // 若 x 在節點上，用數值導數（左右導數平均）
        for i in 0..n {
            if (x - self.x_data[i]).abs() < f64::EPSILON {
                let h = 1e-8;
                let left = self.value_barycentric(x - h);
                let right = self.value_barycentric(x + h);
                return (right - left) / (2.0 * h);
            }
        }

        let mut n_val = 0.0;
        let mut d_val = 0.0;
        let mut n_prime = 0.0;
        let mut d_prime = 0.0;

        for i in 0..n {
            let diff = x - self.x_data[i];
            let temp = self.weights[i] / diff;
            let temp_sq = temp / diff;

            n_val += temp * self.y_data[i];
            d_val += temp;
            n_prime -= temp_sq * self.y_data[i];
            d_prime -= temp_sq;
        }

        // L'(x) = (N'·D - N·D') / D²
        (n_prime * d_val - n_val * d_prime) / (d_val * d_val)
    }

    /// 使用 monomial form 計算定積分
    ///
    /// ∫_a^b Σ c_k·x^k dx = Σ c_k·[x^(k+1)/(k+1)]_a^b
    fn integral_monomial(&self, a: f64, b: f64) -> f64 {
        let coefs = self.monomial_coefs.as_ref().unwrap();
        let n = coefs.len();

        let mut result = 0.0;
        for k in 0..n {
            let power = (k + 1) as f64;
            result += coefs[k] * (b.powi((k + 1) as i32) - a.powi((k + 1) as i32)) / power;
        }

        result
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Trait 實作
// ─────────────────────────────────────────────────────────────────────────────

impl NonparametricCurve for LagrangePolynomial {
    fn points(&self) -> Vec<Point2D> {
        self.x_data
            .iter()
            .zip(self.y_data.iter())
            .map(|(&x, &y)| Point2D::new(x, y))
            .collect()
    }

    fn min_x(&self) -> f64 {
        *self.x_data.first().unwrap()
    }

    fn max_x(&self) -> f64 {
        *self.x_data.last().unwrap()
    }
}

impl Curve for LagrangePolynomial {
    fn value(&self, x: f64) -> f64 {
        self.value_barycentric(x)
    }

    fn derivative(&self, x: f64) -> f64 {
        assert!(
            self.has_derivatives,
            "derivatives not enabled: use new_with_derivatives()"
        );
        self.derivative_barycentric(x)
    }
}

impl CurveIntegration for LagrangePolynomial {
    fn integral(&self, a: f64, b: f64) -> f64 {
        assert!(
            self.monomial_coefs.is_some(),
            "integrals not enabled: use new_with_integrals()"
        );

        // 符號慣例：∫_a^b = -∫_b^a
        if (a - b).abs() < f64::EPSILON {
            return 0.0;
        }

        if a > b {
            -self.integral_monomial(b, a)
        } else {
            self.integral_monomial(a, b)
        }
    }
}
