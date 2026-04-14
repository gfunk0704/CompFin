use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;
use thiserror::Error;

use crate::configuration::InterestRateInstrumentGeneratorCollection;
use crate::instrument::instrument::{Position, SimpleInstrument};
use crate::marketdata::interestrate::interestratequotesheet::{
    InterestRateQuoteSheet,
    InterestRateQuoteSheetError,
};
use crate::model::interestrate::interestratecurve::{
    InterestRateCurve,
    InterestRateCurveGenerator,
};


// ─────────────────────────────────────────────────────────────────────────────
// CalibrationError
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CalibrationError {
    #[error("quote sheet '{0}' not found in quote book")]
    SheetNotFound(String),

    #[error(transparent)]
    QuoteSheet(#[from] InterestRateQuoteSheetError),

    #[error("NthQuote({index}) out of range: sheet '{sheet}' has only {len} quotes")]
    NthQuoteOutOfRange {
        index: usize,
        sheet: String,
        len:   usize,
    },

    #[error("curve generation failed: {0}")]
    CurveGeneration(String),
}


// ─────────────────────────────────────────────────────────────────────────────
// MaturityKey / InterestRateCurvePillar
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq)]
pub enum MaturityKey {
    Tenor(String),
    Date(String),
    NthQuote(usize),
}

pub struct InterestRateCurvePillar {
    maturity_key:         MaturityKey,
    quote_generator_name: String,
}

impl InterestRateCurvePillar {
    pub fn new(maturity_key: MaturityKey, quote_generator_name: String) -> Self {
        Self { maturity_key, quote_generator_name }
    }

    pub fn maturity_key(&self) -> &MaturityKey { &self.maturity_key }
    pub fn quote_generator_name(&self) -> &String { &self.quote_generator_name }
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateCurveCalibrationHelper
// ─────────────────────────────────────────────────────────────────────────────
//
// 泛型參數 T 代表校準商品的持有型別：
//   - 預設 `Arc<dyn SimpleInstrument>`：標準路徑
//   - `FreezableInstrument`：啟用 partial freeze 優化時使用

pub struct InterestRateCurveCalibrationHelper<T = Arc<dyn SimpleInstrument>> {
    instrument: T,
    market_rate: f64,
}

impl<T> InterestRateCurveCalibrationHelper<T> {
    pub fn new(instrument: T, market_rate: f64) -> Self {
        Self { instrument, market_rate }
    }

    pub fn instrument(&self) -> &T { &self.instrument }
    pub fn market_rate(&self) -> f64 { self.market_rate }

    /// 消費 helper，取出 instrument 的所有權。
    pub fn into_instrument(self) -> T { self.instrument }
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateCurveCalibrator
// ─────────────────────────────────────────────────────────────────────────────

pub trait InterestRateCurveCalibrator {
    fn generate_calibration_set(
        pillars:              &[InterestRateCurvePillar],
        quote_book:           &HashMap<String, InterestRateQuoteSheet>,
        generator_collection: &InterestRateInstrumentGeneratorCollection,
        position:             Position,
        horizon:              NaiveDate,
    ) -> Result<Vec<InterestRateCurveCalibrationHelper>, CalibrationError> {
        pillars
            .iter()
            .map(|pillar| {
                let sheet = quote_book
                    .get(pillar.quote_generator_name())
                    .ok_or_else(|| {
                        CalibrationError::SheetNotFound(
                            pillar.quote_generator_name().clone()
                        )
                    })?;

                let key: String = match pillar.maturity_key() {
                    MaturityKey::Tenor(s) | MaturityKey::Date(s) => s.clone(),
                    MaturityKey::NthQuote(n) => {
                        let mut keys: Vec<&String> = sheet.keys().collect();
                        keys.sort();
                        keys.get(*n)
                            .map(|k| k.to_string())
                            .ok_or_else(|| CalibrationError::NthQuoteOutOfRange {
                                index: *n,
                                sheet: pillar.quote_generator_name().clone(),
                                len:   keys.len(),
                            })?
                    }
                };

                sheet
                    .generate_calibration_helper(&key, position, horizon, generator_collection)
                    .map_err(CalibrationError::from)
            })
            .collect()
    }

    fn calibrate(
        &self,
        curve_generator:      Arc<dyn InterestRateCurveGenerator>,
        reference_date:       NaiveDate,
        pillars:              Vec<InterestRateCurvePillar>,
        quote_book:           &HashMap<String, InterestRateQuoteSheet>,
        generator_collection: &InterestRateInstrumentGeneratorCollection,
        position:             Position,
        horizon:              NaiveDate,
    ) -> Result<Arc<dyn InterestRateCurve>, CalibrationError>;
}
