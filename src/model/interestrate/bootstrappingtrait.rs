// ── bootstrappingtrait.rs ─────────────────────────────────────────────────────
//
// Bootstrapping 用的輔助結構，負責：
//   - 根據 InterpolationTarget 產生 root solver 的初始猜測值
//   - 產生 bracket 的上下界
//
// # 初值邏輯
//
//   ZeroRate / InstForwardRate 目標：
//     initial_value = market_rate（利率本身就是合理的起始點）
//
//   LogDiscount 目標：
//     initial_value = -market_rate × τ
//     其中 τ 為 reference_date 至 pillar_date 的年化時間（使用曲線自身的 day counter）。
//     數學推導：ln(D(t)) ≈ -r × t（平坦曲線假設）
//
// # Bracket 邏輯
//
//   margin = max(|initial_value| × 0.5, 1e-4)
//   lower  = initial_value − margin
//   upper  = initial_value + margin
//
//   1e-4 的最低保障確保零利率或極低利率環境下 bracket 不退化。

use crate::model::interestrate::interestratecurve::YearFractionCalculator;
use crate::model::interestrate::piecewisepolyinterestratecurve::InterpolationTarget;

use chrono::NaiveDate;


// ─────────────────────────────────────────────────────────────────────────────
// BootstrappingTrait
// ─────────────────────────────────────────────────────────────────────────────

pub struct BootstrappingTrait {
    interpolation_target: InterpolationTarget,
}

impl BootstrappingTrait {
    pub fn new(interpolation_target: InterpolationTarget) -> Self {
        Self { interpolation_target }
    }

    /// 根據 InterpolationTarget 產生 root solver 的初始猜測值。
    ///
    /// - `market_rate`: 已經過 `SimpleInstrumentGenerator::market_rate()` 轉換的等效利率
    /// - `yfc`: 曲線的 YearFractionCalculator（提供 reference_date 與 day counter）
    /// - `pillar_date`: 該 pillar 的到期日
    pub fn initial_value(
        &self,
        market_rate: f64,
        yfc:         &YearFractionCalculator,
        pillar_date: NaiveDate,
    ) -> f64 {
        match self.interpolation_target {
            InterpolationTarget::ZeroRate |
            InterpolationTarget::InstantaneousForwardRate => market_rate,
            InterpolationTarget::LogDiscount => {
                let tau = yfc.year_fraction(pillar_date);
                -market_rate * tau
            }
        }
    }

    /// 產生 bracket 的下界與上界。
    ///
    /// 使用 `max(|initial| × 0.5, 1e-4)` 作為 margin，
    /// 確保零利率、負利率環境下 bracket 不退化。
    pub fn bracket(&self, initial_value: f64) -> (f64, f64) {
        let half = initial_value.abs() * 0.5;
        let margin = half.max(1e-4);
        (initial_value - margin, initial_value + margin)
    }

    /// 將 FlatForwardCurve 求出的 zero rate 轉換為
    /// PiecewisePoly 的 InterpolationTarget 所需的值。
    ///
    /// FlatForwardCurve 解出的是常數利率 r（= zero rate = inst forward），
    /// 若目標曲線的 InterpolationTarget 是 LogDiscount，
    /// 需要轉換為 ln(D(t)) = -r × τ。
    ///
    /// - `solved_rate`: FlatForwardCurve 求出的常數利率
    /// - `yfc`: 曲線的 YearFractionCalculator
    /// - `pillar_date`: 第一個 pillar 的到期日
    pub fn convert_flat_forward_to_target(
        &self,
        solved_rate: f64,
        yfc:         &YearFractionCalculator,
        pillar_date: NaiveDate,
    ) -> f64 {
        match self.interpolation_target {
            InterpolationTarget::ZeroRate |
            InterpolationTarget::InstantaneousForwardRate => solved_rate,
            InterpolationTarget::LogDiscount => {
                let tau = yfc.year_fraction(pillar_date);
                -solved_rate * tau
            }
        }
    }
}
