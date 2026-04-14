use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use chrono::NaiveDate;

use crate::configuration::InterestRateInstrumentGeneratorCollection;
use crate::instrument::instrument::{Position, SimpleInstrument};
use crate::instrument::interestrate::simpleinterestrateinstrumentgenerator::SimpleInterestRateInstrumentGenerator;
use crate::manager::managererror::ManagerError;
use crate::model::interestrate::interestratecurvecalibrator::InterestRateCurveCalibrationHelper;
use crate::time::period::Period;


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateQuoteSheetError
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum InterestRateQuoteSheetError {
    #[error(transparent)]
    Manager(#[from] ManagerError),

    #[error("maturity not found in sheet: {0}")]
    MaturityNotFound(String),

    #[error("failed to parse tenor key \"{0}\": {1}")]
    TenorParse(String, String),

    #[error("failed to parse date key \"{0}\": {1}")]
    DateParse(String, String),

    #[error("instrument generation failed: {0}")]
    InstrumentGeneration(String),
}


// ─────────────────────────────────────────────────────────────────────────────
// Quote routing enums
// ─────────────────────────────────────────────────────────────────────────────

pub enum InterestRateSwapQuoteLeg {
    PayLeg,
    ReceiveLeg,
}

pub enum InterestRateSwapQuoteTarget {
    ParRate,
    Spread,
}

/// 決定如何將 quote 值 apply 到 generator，以及用哪種 key 格式產生 instrument。
///
/// # Key 格式慣例
/// - `Deposit` / `InterestRateSwap`：key 為 tenor 字串，如 `"3M"`、`"1Y"`
/// - `Future`（未來擴充）：key 為日期字串，如 `"2024-06-15"`
pub enum InterestRateGeneratorType {
    Deposit,
    InterestRateSwap {
        leg:    InterestRateSwapQuoteLeg,
        target: InterestRateSwapQuoteTarget,
    },
}


// ─────────────────────────────────────────────────────────────────────────────
// InterestRateQuoteSheet
// ─────────────────────────────────────────────────────────────────────────────
//
// 命名說明：
//   「Quote Sheet」而非「Rate Sheet」，是因為這裡存的是市場直接報出的 quote 值，
//   而非經過轉換的 rate（例如 future 的 market rate = 100 - quote，
//   FX swap point 的 implied rate 也需要另外計算）。
//   轉換邏輯由各自的商品型別負責，不在此處理。

pub struct InterestRateQuoteSheet {
    generator_name: String,
    generator_type: InterestRateGeneratorType,
    /// key 統一使用 String：tenor 用 "3M" / "1Y"，到期日用 "2024-06-15"。
    sheet:          HashMap<String, f64>,
}

impl InterestRateQuoteSheet {
    pub fn new(generator_name: String, generator_type: InterestRateGeneratorType) -> Self {
        Self {
            generator_name,
            generator_type,
            sheet: HashMap::new(),
        }
    }

    pub fn add_quote(&mut self, key: impl Into<String>, value: f64) {
        self.sheet.insert(key.into(), value);
    }

    pub fn get_quote(&self, key: &str) -> Option<&f64> {
        self.sheet.get(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.sheet.keys()
    }

    pub fn generator_name(&self) -> &str {
        &self.generator_name
    }

    pub fn generator_type(&self) -> &InterestRateGeneratorType {
        &self.generator_type
    }

    /// 取得 quote 值、apply 到 generator setter、產生 instrument。
    ///
    /// # 參數
    /// - `key`: sheet 中的 key，tenor 格式（`"3M"`）或日期格式（`"2024-06-15"`）
    /// - `position`: 產生 instrument 時的部位
    /// - `trade_date`: 交易日
    /// - `generator_collection`: 包含所有 instrument generator 的集合
    pub fn generate_instrument(
        &self,
        key:                  &str,
        position:             Position,
        trade_date:           NaiveDate,
        generator_collection: &InterestRateInstrumentGeneratorCollection,
    ) -> Result<Arc<dyn SimpleInstrument>, InterestRateQuoteSheetError> {
        let quote = self
            .sheet
            .get(key)
            .ok_or_else(|| InterestRateQuoteSheetError::MaturityNotFound(key.to_string()))?;

        match &self.generator_type {
            InterestRateGeneratorType::Deposit => {
                let generator = generator_collection
                    .deposit_generator_manager
                    .get(&self.generator_name)?;

                generator
                    .leg_character_genrator()
                    .setter()
                    .set_fixed_rate(*quote);

                let tenor = Period::parse(key).map_err(|e| {
                    InterestRateQuoteSheetError::TenorParse(key.to_string(), e.to_string())
                })?;
                generator
                    .generate_with_maturity_tenor(position, trade_date, tenor, None)
                    .map_err(InterestRateQuoteSheetError::InstrumentGeneration)
            }

            InterestRateGeneratorType::InterestRateSwap { leg, target } => {
                let generator = generator_collection
                    .swap_generator_manager
                    .get(&self.generator_name)?;

                let setter = match leg {
                    InterestRateSwapQuoteLeg::PayLeg =>
                        generator.pay_leg_character_genrator().setter(),
                    InterestRateSwapQuoteLeg::ReceiveLeg =>
                        generator.receive_leg_character_genrator().setter(),
                };

                match target {
                    InterestRateSwapQuoteTarget::ParRate =>
                        setter.set_fixed_rate(*quote),
                    InterestRateSwapQuoteTarget::Spread =>
                        setter.set_spread(*quote),
                }

                let tenor = Period::parse(key).map_err(|e| {
                    InterestRateQuoteSheetError::TenorParse(key.to_string(), e.to_string())
                })?;
                generator
                    .generate_with_maturity_tenor(position, trade_date, tenor, None)
                    .map_err(InterestRateQuoteSheetError::InstrumentGeneration)
            }
        }
    }

    /// 取得 quote 值、apply 到 generator、產生 instrument，
    /// 並透過 generator 的 `market_rate()` 轉換出 bootstrapping 用的等效利率，
    /// 一起包裝成 [`InterestRateCurveCalibrationHelper`]。
    ///
    /// 此方法供 `generate_calibration_set` 使用，確保 instrument 與 market_rate
    /// 在同一個流程中配對產生，不會遺失轉換資訊。
    pub fn generate_calibration_helper(
        &self,
        key:                  &str,
        position:             Position,
        trade_date:           NaiveDate,
        generator_collection: &InterestRateInstrumentGeneratorCollection,
    ) -> Result<InterestRateCurveCalibrationHelper, InterestRateQuoteSheetError> {
        let quote = *self
            .sheet
            .get(key)
            .ok_or_else(|| InterestRateQuoteSheetError::MaturityNotFound(key.to_string()))?;

        match &self.generator_type {
            InterestRateGeneratorType::Deposit => {
                let generator = generator_collection
                    .deposit_generator_manager
                    .get(&self.generator_name)?;

                generator
                    .leg_character_genrator()
                    .setter()
                    .set_fixed_rate(quote);

                let market_rate = generator.market_rate(quote);

                let tenor = Period::parse(key).map_err(|e| {
                    InterestRateQuoteSheetError::TenorParse(key.to_string(), e.to_string())
                })?;
                let instrument = generator
                    .generate_with_maturity_tenor(position, trade_date, tenor, None)
                    .map_err(InterestRateQuoteSheetError::InstrumentGeneration)?;

                Ok(InterestRateCurveCalibrationHelper::new(instrument, market_rate))
            }

            InterestRateGeneratorType::InterestRateSwap { leg, target } => {
                let generator = generator_collection
                    .swap_generator_manager
                    .get(&self.generator_name)?;

                let setter = match leg {
                    InterestRateSwapQuoteLeg::PayLeg =>
                        generator.pay_leg_character_genrator().setter(),
                    InterestRateSwapQuoteLeg::ReceiveLeg =>
                        generator.receive_leg_character_genrator().setter(),
                };

                match target {
                    InterestRateSwapQuoteTarget::ParRate =>
                        setter.set_fixed_rate(quote),
                    InterestRateSwapQuoteTarget::Spread =>
                        setter.set_spread(quote),
                }

                let market_rate = generator.market_rate(quote);

                let tenor = Period::parse(key).map_err(|e| {
                    InterestRateQuoteSheetError::TenorParse(key.to_string(), e.to_string())
                })?;
                let instrument = generator
                    .generate_with_maturity_tenor(position, trade_date, tenor, None)
                    .map_err(InterestRateQuoteSheetError::InstrumentGeneration)?;

                Ok(InterestRateCurveCalibrationHelper::new(instrument, market_rate))
            }
        }
    }
}