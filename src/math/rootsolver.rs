// ── rootsolver.rs ─────────────────────────────────────────────────────────────
//
// 一般化的 1D root solver。
//
// # 設計說明
//
// 呼叫端提供一個或兩個初始點，RootSolver 依照以下邏輯自動選擇演算法：
//
//   兩個初始點，且 f(x0) 與 f(x1) 異號
//     → Bracketing（Brent 或 Ridder）：有界、保證收斂
//
//   其他情況（同號，或只有一個初始點）
//     → QuasiNewton（Steffensen 或 Secant）：需要初值夠近，收斂快
//
// QuasiNewton 的起始點選擇：
//   - Steffensen：用 |f(x)| 較小的點，或唯一的點
//   - Secant：用兩個點（若只有一個點，第二個用 x0 + 1e-4 補）
//
// # 收斂判斷
//
// |f(x)| < tolerance（函數值絕對值，而非步長），與 Python 版保持一致。

use thiserror::Error;


// ─────────────────────────────────────────────────────────────────────────────
// RootSolverError
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum RootSolverError {
    #[error("bracketing failed: f(x0) and f(x1) have the same sign")]
    NotBracketed,

    #[error("solver did not converge within {max_iter} iterations")]
    NotConverged { max_iter: usize },

    #[error("derivative approximation is zero; cannot continue")]
    ZeroDerivative,
}


// ─────────────────────────────────────────────────────────────────────────────
// 方法選擇
// ─────────────────────────────────────────────────────────────────────────────

/// 有初始 bracket 時使用的方法。
#[derive(Clone, Copy, Debug)]
pub enum BracketingMethod {
    /// Brent–Dekker method：結合 bisection、secant、inverse quadratic interpolation。
    /// 保證在 bracket 內收斂，通常為超線性。
    Brent,
    /// Ridder's method：每步計算兩次函數值，收斂階為 √2。
    Ridder,
}

impl Default for BracketingMethod {
    fn default() -> Self {
        BracketingMethod::Brent
    }
}

/// 無 bracket 時使用的方法。
#[derive(Clone, Copy, Debug)]
pub enum QuasiNewtonMethod {
    /// Steffensen's method：quadratic convergence，每步兩次函數求值，只需一個初始點。
    Steffensen,
    /// Secant method：收斂階 ~1.618，每步一次函數求值，需要兩個初始點。
    /// 若只給一個初始點，自動補 x0 + 1e-4。
    Secant,
}

impl Default for QuasiNewtonMethod {
    fn default() -> Self {
        QuasiNewtonMethod::Steffensen
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// RootSolverConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Root solver 的參數設定。
///
/// 使用 [`RootSolverConfig::default()`] 取得合理的預設值，再按需覆蓋。
#[derive(Clone, Debug)]
pub struct RootSolverConfig {
    pub bracketing:   BracketingMethod,
    pub quasi_newton: QuasiNewtonMethod,
    /// 收斂判斷：|f(x)| < tolerance。
    pub tolerance:    f64,
    pub max_iter:     usize,
    /// QuasiNewton 失敗後，bracket 搜尋的最大展開次數。
    /// 每次從起始點往兩側對稱擴展，步長乘以 `bracket_expansion_factor`。
    pub bracket_search_max_iter: usize,
    /// Bracket 搜尋時每步的步長擴展倍數，預設 1.6（QuantLib 慣例）。
    pub bracket_expansion_factor: f64,
}

impl Default for RootSolverConfig {
    fn default() -> Self {
        Self {
            bracketing:               BracketingMethod::default(),
            quasi_newton:             QuasiNewtonMethod::default(),
            tolerance:                1e-12,
            max_iter:                 100,
            bracket_search_max_iter:  50,
            bracket_expansion_factor: 1.6,
        }
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// RootSolver
// ─────────────────────────────────────────────────────────────────────────────

pub struct RootSolver {
    config: RootSolverConfig,
}

impl RootSolver {
    pub fn new(config: RootSolverConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(RootSolverConfig::default())
    }

    /// 求解 f(x) = 0。
    ///
    /// # 策略
    ///
    /// 1. 若提供兩個初始點且異號，直接使用 Bracketing（最穩定）。
    /// 2. 否則先嘗試 QuasiNewton（快速，需要初值夠近）。
    /// 3. QuasiNewton 失敗後，從起始點展開搜尋找到異號區間，再使用 Bracketing（fallback）。
    ///
    /// 這個設計適合初值有金融意義（如 par rate）的場景：
    /// 正常情況下 QuasiNewton 直接收斂，不需要額外的函數求值；
    /// 例外情況下 Bracketing fallback 保證找到根。
    pub fn solve<F>(&self, f: F, x0: f64, x1_opt: Option<f64>) -> Result<f64, RootSolverError>
    where
        F: Fn(f64) -> f64,
    {
        let fx0 = f(x0);
        if fx0.abs() < self.config.tolerance {
            return Ok(x0);
        }

        // 若提供兩個點且已異號，直接進 Bracketing，不需要 QuasiNewton
        if let Some(x1) = x1_opt {
            let fx1 = f(x1);
            if fx1.abs() < self.config.tolerance {
                return Ok(x1);
            }
            if fx0 * fx1 < 0.0 {
                return self.bracketing(&f, x0, fx0, x1, fx1);
            }
        }

        // QuasiNewton 第一次嘗試
        // Steffensen：用 |f| 較小的點；Secant：兩個點都用（若只有一個補 x0 + 1e-4）
        let qn_result = match x1_opt {
            Some(x1) => {
                let fx1 = f(x1);
                let (start, _) = if fx0.abs() <= fx1.abs() {
                    (x0, fx0)
                } else {
                    (x1, fx1)
                };
                match self.config.quasi_newton {
                    QuasiNewtonMethod::Steffensen => self.steffensen(&f, start),
                    QuasiNewtonMethod::Secant     => self.secant(&f, x0, fx0, x1, fx1),
                }
            }
            None => match self.config.quasi_newton {
                QuasiNewtonMethod::Steffensen => self.steffensen(&f, x0),
                QuasiNewtonMethod::Secant => {
                    let x1  = x0 + 1e-4;
                    let fx1 = f(x1);
                    self.secant(&f, x0, fx0, x1, fx1)
                }
            },
        };

        // QuasiNewton 成功則直接回傳
        if qn_result.is_ok() {
            return qn_result;
        }

        // Fallback：從 x0 展開搜尋，找到異號區間後進 Bracketing
        self.bracket_search_and_solve(&f, x0, fx0)
    }


    // ─────────────────────────────────────────────────────────────────────────
    // Bracket 搜尋 + Bracketing fallback
    // ─────────────────────────────────────────────────────────────────────────
    //
    // 從 x0 出發，以初始步長 |x0| * 0.01（或 0.01 若 x0 接近零）往兩側對稱展開，
    // 每步乘以 expansion_factor，直到找到異號區間。
    // 找到後交給 Bracketing 方法求根。

    fn bracket_search_and_solve<F>(
        &self,
        f: &F,
        x0: f64,
        fx0: f64,
    ) -> Result<f64, RootSolverError>
    where
        F: Fn(f64) -> f64,
    {
        let mut step = if x0.abs() > 1e-4 { x0.abs() * 0.01 } else { 0.01 };
        let factor   = self.config.bracket_expansion_factor;

        for _ in 0..self.config.bracket_search_max_iter {
            // 往右展開
            let xr  = x0 + step;
            let fxr = f(xr);
            if fx0 * fxr < 0.0 {
                return self.bracketing(f, x0, fx0, xr, fxr);
            }

            // 往左展開
            let xl  = x0 - step;
            let fxl = f(xl);
            if fx0 * fxl < 0.0 {
                return self.bracketing(f, xl, fxl, x0, fx0);
            }

            step *= factor;
        }

        Err(RootSolverError::NotConverged {
            max_iter: self.config.bracket_search_max_iter,
        })
    }

    /// Bracketing 方法的統一入口。
    fn bracketing<F>(
        &self,
        f: &F,
        a: f64, fa: f64,
        b: f64, fb: f64,
    ) -> Result<f64, RootSolverError>
    where
        F: Fn(f64) -> f64,
    {
        match self.config.bracketing {
            BracketingMethod::Brent  => self.brent(f, a, fa, b, fb),
            BracketingMethod::Ridder => self.ridder(f, a, fa, b, fb),
        }
    }


    // ─────────────────────────────────────────────────────────────────────────
    // Steffensen's method
    // ─────────────────────────────────────────────────────────────────────────
    //
    // 每步兩次函數求值：f(x) 和 f(x + f(x))。
    // 用有限差分 [f(x + f(x)) - f(x)] / f(x) 近似 f'(x)，達到 quadratic convergence。

    fn steffensen<F>(&self, f: &F, x0: f64) -> Result<f64, RootSolverError>
    where
        F: Fn(f64) -> f64,
    {
        let mut x = x0;
        for _ in 0..self.config.max_iter {
            let fx = f(x);
            if fx.abs() < self.config.tolerance {
                return Ok(x);
            }
            let denom = f(x + fx) - fx;
            if denom.abs() < f64::EPSILON {
                return Err(RootSolverError::ZeroDerivative);
            }
            x -= fx * fx / denom;
        }
        Err(RootSolverError::NotConverged { max_iter: self.config.max_iter })
    }


    // ─────────────────────────────────────────────────────────────────────────
    // Secant method
    // ─────────────────────────────────────────────────────────────────────────
    //
    // 每步一次函數求值，收斂階 ~1.618（黃金比例）。
    // 用前兩步的差分近似導數，不需要解析式。

    fn secant<F>(
        &self,
        f: &F,
        mut x0: f64, mut fx0: f64,
        mut x1: f64, mut fx1: f64,
    ) -> Result<f64, RootSolverError>
    where
        F: Fn(f64) -> f64,
    {
        for _ in 0..self.config.max_iter {
            if fx1.abs() < self.config.tolerance {
                return Ok(x1);
            }
            let denom = fx1 - fx0;
            if denom.abs() < f64::EPSILON {
                return Err(RootSolverError::ZeroDerivative);
            }
            let x2  = x1 - fx1 * (x1 - x0) / denom;
            let fx2 = f(x2);
            x0  = x1;  fx0 = fx1;
            x1  = x2;  fx1 = fx2;
        }
        Err(RootSolverError::NotConverged { max_iter: self.config.max_iter })
    }


    // ─────────────────────────────────────────────────────────────────────────
    // Brent's method
    // ─────────────────────────────────────────────────────────────────────────
    //
    // 結合 bisection（保證收斂）、secant 和 inverse quadratic interpolation（加速）。
    // 在有 bracket 的情況下是最常用的保守選擇。
    //
    // 實作依照 Brent (1973) 的原始描述。

    fn brent<F>(
        &self,
        f: &F,
        mut a: f64, mut fa: f64,
        mut b: f64, mut fb: f64,
    ) -> Result<f64, RootSolverError>
    where
        F: Fn(f64) -> f64,
    {
        // 確保 |f(b)| <= |f(a)|
        if fa.abs() < fb.abs() {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut fa, &mut fb);
        }

        let mut c  = a;
        let mut fc = fa;
        let mut mflag = true;
        let mut s  = 0.0;
        let mut d  = 0.0;

        for _ in 0..self.config.max_iter {
            if fb.abs() < self.config.tolerance {
                return Ok(b);
            }
            if (a - b).abs() < f64::EPSILON {
                return Ok(b);
            }

            if fa != fc && fb != fc {
                // Inverse quadratic interpolation
                s = a * fb * fc / ((fa - fb) * (fa - fc))
                  + b * fa * fc / ((fb - fa) * (fb - fc))
                  + c * fa * fb / ((fc - fa) * (fc - fb));
            } else {
                // Secant
                s = b - fb * (b - a) / (fb - fa);
            }

            let cond1 = !is_between(s, (3.0 * a + b) / 4.0, b);
            let cond2 =  mflag && (s - b).abs() >= (b - c).abs() / 2.0;
            let cond3 = !mflag && (s - b).abs() >= (c - d).abs() / 2.0;
            let cond4 =  mflag && (b - c).abs() < self.config.tolerance;
            let cond5 = !mflag && (c - d).abs() < self.config.tolerance;

            if cond1 || cond2 || cond3 || cond4 || cond5 {
                // Bisection fallback
                s = (a + b) / 2.0;
                mflag = true;
            } else {
                mflag = false;
            }

            let fs = f(s);
            d = c;
            c  = b;
            fc = fb;

            if fa * fs < 0.0 {
                b  = s;
                fb = fs;
            } else {
                a  = s;
                fa = fs;
            }

            if fa.abs() < fb.abs() {
                std::mem::swap(&mut a, &mut b);
                std::mem::swap(&mut fa, &mut fb);
            }
        }
        Err(RootSolverError::NotConverged { max_iter: self.config.max_iter })
    }


    // ─────────────────────────────────────────────────────────────────────────
    // Ridder's method
    // ─────────────────────────────────────────────────────────────────────────
    //
    // 每步計算兩次函數值，收斂階為 √2 ≈ 1.414。
    // 比 bisection 快，比 Brent 在某些情況下更簡單。

    fn ridder<F>(
        &self,
        f: &F,
        mut a: f64, mut fa: f64,
        mut b: f64, mut fb: f64,
    ) -> Result<f64, RootSolverError>
    where
        F: Fn(f64) -> f64,
    {
        for _ in 0..self.config.max_iter {
            let m   = (a + b) / 2.0;
            let fm  = f(m);

            if fm.abs() < self.config.tolerance {
                return Ok(m);
            }

            let sign = if fa - fb < 0.0 { -1.0 } else { 1.0 };
            let denom = (fm * fm - fa * fb).sqrt();
            if denom.abs() < f64::EPSILON {
                return Err(RootSolverError::ZeroDerivative);
            }

            let x  = m + (m - a) * sign * fm / denom;
            let fx = f(x);

            if fx.abs() < self.config.tolerance {
                return Ok(x);
            }

            // 更新 bracket
            if fm * fx < 0.0 {
                a  = m;  fa = fm;
                b  = x;  fb = fx;
            } else if fa * fx < 0.0 {
                b  = x;  fb = fx;
            } else {
                a  = x;  fa = fx;
            }

            if (b - a).abs() < self.config.tolerance {
                return Ok(x);
            }
        }
        Err(RootSolverError::NotConverged { max_iter: self.config.max_iter })
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// 輔助函式
// ─────────────────────────────────────────────────────────────────────────────

/// s 是否在 a 與 b 之間（含端點，不要求 a < b）。
fn is_between(s: f64, a: f64, b: f64) -> bool {
    if a < b { a <= s && s <= b }
    else      { b <= s && s <= a }
}
