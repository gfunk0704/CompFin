// ── flatforwardcurve.rs ───────────────────────────────────────────────────────
//
// 假設瞬時遠期利率（instantaneous forward rate）為常數的利率曲線。
//
// # 數學定義
//
//   f(t) = r           （常數）
//   R(t) = r           （zero rate = forward rate under flat forward）
//   D(t) = exp(-r × t) （折現因子）
//
// # 用途
//
// 在 IterativeBootstrapping 中，第一個 pillar 只有一個校準商品，
// 無法用 PiecewisePolynomial（至少需要兩個點）建構曲線。
// FlatForwardCurve 只需一個參數（常數利率 r），是此情境下
// 數學上最合理的選擇。
//
// 當左外插設定為 FlatForwardRate 時，第二個 pillar 開始切換回
// PiecewisePoly 後，t < t_1 區間的行為與 FlatForwardCurve 完全一致，
// 保證曲線的連續性與無套利。

use std::sync::Arc;

use chrono::NaiveDate;

use crate::model::interestrate::interestratecurve::{
    DiscountCurve,
    InstForwardCurve,
    InterestRateCurve,
    YearFractionCalculator,
    ZeroRateCurve,
};


// ─────────────────────────────────────────────────────────────────────────────
// FlatForwardCurve
// ─────────────────────────────────────────────────────────────────────────────

pub struct FlatForwardCurve {
    yfc:  YearFractionCalculator,
    rate: f64,
}

impl FlatForwardCurve {
    pub fn new(yfc: YearFractionCalculator, rate: f64) -> Self {
        Self { yfc, rate }
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }
}

impl InterestRateCurve for FlatForwardCurve {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator { &self.yfc }

    fn to_discount_curve(&self) -> Arc<dyn DiscountCurve> {
        Arc::new(FlatForwardDiscountCurve { yfc: self.yfc.clone(), rate: self.rate })
    }

    fn to_zero_rate_curve(&self) -> Arc<dyn ZeroRateCurve> {
        Arc::new(FlatForwardZeroRateCurve { yfc: self.yfc.clone(), rate: self.rate })
    }

    fn to_inst_forward_curve(&self) -> Arc<dyn InstForwardCurve> {
        Arc::new(FlatForwardInstForwardCurve { yfc: self.yfc.clone(), rate: self.rate })
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// Sub-curve implementations
// ─────────────────────────────────────────────────────────────────────────────

struct FlatForwardDiscountCurve {
    yfc:  YearFractionCalculator,
    rate: f64,
}

impl DiscountCurve for FlatForwardDiscountCurve {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator { &self.yfc }

    fn discount(&self, d: NaiveDate) -> f64 {
        let t = self.yfc.year_fraction(d);
        (-self.rate * t).exp()
    }
}


struct FlatForwardZeroRateCurve {
    yfc:  YearFractionCalculator,
    rate: f64,
}

impl ZeroRateCurve for FlatForwardZeroRateCurve {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator { &self.yfc }

    fn zero_rate(&self, _d: NaiveDate) -> f64 {
        self.rate
    }
}


struct FlatForwardInstForwardCurve {
    yfc:  YearFractionCalculator,
    rate: f64,
}

impl InstForwardCurve for FlatForwardInstForwardCurve {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator { &self.yfc }

    fn inst_forward(&self, _d: NaiveDate) -> f64 {
        self.rate
    }
}
