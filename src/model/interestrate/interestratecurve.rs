use std::sync::Arc;

use chrono::NaiveDate;

use crate::time::daycounter::daycounter::DayCounter;


// ─────────────────────────────────────────────────────────────────────────────
// YearFractionCalculator
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct YearFractionCalculator {
    reference_date: NaiveDate,
    day_counter:    Arc<DayCounter>,
}

impl YearFractionCalculator {
    pub fn new(reference_date: NaiveDate, day_counter: Arc<DayCounter>) -> Self {
        Self { reference_date, day_counter }
    }

    pub fn reference_date(&self) -> NaiveDate { self.reference_date }
    pub fn day_counter(&self) -> &Arc<DayCounter> { &self.day_counter }

    pub fn year_fraction(&self, d: NaiveDate) -> f64 {
        self.day_counter.year_fraction(self.reference_date, d)
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// 共用 default methods macro
// ─────────────────────────────────────────────────────────────────────────────

macro_rules! impl_curve_common_methods {
    () => {
        fn reference_date(&self) -> NaiveDate {
            self.year_fraction_calculator().reference_date()
        }

        fn day_counter(&self) -> &Arc<DayCounter> {
            self.year_fraction_calculator().day_counter()
        }

        fn year_fraction(&self, d: NaiveDate) -> f64 {
            self.year_fraction_calculator().year_fraction(d)
        }
    };
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateCurve
// ─────────────────────────────────────────────────────────────────────────────

pub trait InterestRateCurve: Send + Sync {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator;

    fn to_discount_curve(&self)     -> Arc<dyn DiscountCurve>;
    fn to_zero_rate_curve(&self)    -> Arc<dyn ZeroRateCurve>;
    fn to_inst_forward_curve(&self) -> Arc<dyn InstForwardCurve>;

    impl_curve_common_methods!();
}


// ─────────────────────────────────────────────────────────────────────────────
// DiscountCurve
// ─────────────────────────────────────────────────────────────────────────────

pub trait DiscountCurve: Send + Sync {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator;
    fn discount(&self, d: NaiveDate) -> f64;
    impl_curve_common_methods!();
}


// ─────────────────────────────────────────────────────────────────────────────
// ZeroRateCurve
// ─────────────────────────────────────────────────────────────────────────────

pub trait ZeroRateCurve: Send + Sync {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator;
    fn zero_rate(&self, d: NaiveDate) -> f64;
    impl_curve_common_methods!();
}


// ─────────────────────────────────────────────────────────────────────────────
// InstForwardCurve
// ─────────────────────────────────────────────────────────────────────────────

pub trait InstForwardCurve: Send + Sync {
    fn year_fraction_calculator(&self) -> &YearFractionCalculator;
    fn inst_forward(&self, d: NaiveDate) -> f64;
    impl_curve_common_methods!();
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateCurveGenerator
// ─────────────────────────────────────────────────────────────────────────────
//
// Curve 的工廠 trait，負責從 calibration values 建立 InterestRateCurve。
// reference_date 在 generate() 時才傳入，讓同一個 generator 可以在不同評價日使用。
// 額外的 supports（如 pillar dates）由各具體 struct 自行暴露獨立的方法。

pub trait InterestRateCurveGenerator: Send + Sync {
    fn generate(
        &self,
        reference_date: NaiveDate,
        values:         Vec<f64>,
    ) -> Result<Arc<dyn InterestRateCurve>, crate::model::interestrate::curvegenerationerror::CurveGenerationError>;

    /// 用指定的 dates 產生 curve，供 calibrator 在逐步建構曲線時使用。
    /// 預設委派到 generate()（此時忽略 dates 參數，使用 generator 內部的 dates）。
    /// 具體實作（如 PiecewisePolyInterestRateCurveGenerator）應覆寫此方法。
    fn generate_with_dates(
        &self,
        reference_date: NaiveDate,
        dates:          &[NaiveDate],
        values:         Vec<f64>,
    ) -> Result<Arc<dyn InterestRateCurve>, crate::model::interestrate::curvegenerationerror::CurveGenerationError> {
        let _ = dates; // 預設忽略 dates
        self.generate(reference_date, values)
    }
}
