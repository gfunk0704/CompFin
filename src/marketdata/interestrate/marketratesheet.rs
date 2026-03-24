use std::fmt;
use std::hash::Hash;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use chrono::NaiveDate;

use crate::configuration::InterestRateInstrumentGeneratorCollection;
use crate::instrument::instrument::{Position, SimpleInstrument};
use crate::instrument::interestrate::simpleinterestrateinstrumentgenerator::SimpleInterestRateInstrumentGenerator;
use crate::manager::managererror::ManagerError;
use crate::time::period::Period;


// ─────────────────────────────────────────────────────────────────────────────
// TenorKey
// ─────────────────────────────────────────────────────────────────────────────
//
// Period 本身沒有 Eq / Hash（不同寫法的相同 tenor 在沒有基準日和 calendar 的情況下
// 無法比較），因此用字串 newtype 作為 HashMap key。
//
// "3M" == "3M" 在字串層次是確定的，比較語意由呼叫端負責確保 key 的一致性
// （例如統一用 Period::to_string() 產生 key）。
// 需要產生 instrument 時才把字串 parse 回 Period。

#[derive(Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct TenorKey(pub String);

impl TenorKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 解析回 [`Period`]，供 generate_instrument 呼叫時使用。
    fn to_period(&self) -> Result<Period, MarketRateSheetError> {
        Period::parse(&self.0).map_err(|e| {
            MarketRateSheetError::TenorParse(format!("{}: {}", self.0, e))
        })
    }
}

impl fmt::Display for TenorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// MarketRateSheetError
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum MarketRateSheetError {
    #[error(transparent)]
    Manager(#[from] ManagerError),

    #[error("maturity not found in sheet: {0}")]
    MaturityNotFound(String),

    #[error("failed to parse tenor key: {0}")]
    TenorParse(String),

    #[error("instrument generation failed: {0}")]
    InstrumentGeneration(String),
}


// ─────────────────────────────────────────────────────────────────────────────
// Maturity trait
// ─────────────────────────────────────────────────────────────────────────────
//
// 封裝 key 型別到 generate 方法的 dispatch：
//   TenorKey  → parse 回 Period → generate_with_maturity_tenor
//   NaiveDate → generate_with_maturity_date

trait Maturity: Eq + Hash + fmt::Display {
    fn generate(
        &self,
        position:   Position,
        trade_date: NaiveDate,
        generator:  &dyn SimpleInterestRateInstrumentGenerator,
    ) -> Result<Arc<dyn SimpleInstrument>, MarketRateSheetError>;
}

impl Maturity for TenorKey {
    fn generate(
        &self,
        position:   Position,
        trade_date: NaiveDate,
        generator:  &dyn SimpleInterestRateInstrumentGenerator,
    ) -> Result<Arc<dyn SimpleInstrument>, MarketRateSheetError> {
        let tenor = self.to_period()?;
        generator
            .generate_with_maturity_tenor(position, trade_date, tenor, None)
            .map_err(MarketRateSheetError::InstrumentGeneration)
    }
}

impl Maturity for NaiveDate {
    fn generate(
        &self,
        position:   Position,
        trade_date: NaiveDate,
        generator:  &dyn SimpleInterestRateInstrumentGenerator,
    ) -> Result<Arc<dyn SimpleInstrument>, MarketRateSheetError> {
        generator
            .generate_with_maturity_date(position, trade_date, *self, None)
            .map_err(MarketRateSheetError::InstrumentGeneration)
    }
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

pub enum InterestRateGeneratorType {
    Deposit,
    InterestRateSwap {
        leg:    InterestRateSwapQuoteLeg,
        target: InterestRateSwapQuoteTarget,
    },
}


// ─────────────────────────────────────────────────────────────────────────────
// MarketRateSheet
// ─────────────────────────────────────────────────────────────────────────────
//
// 型別別名供外部使用：
//   MarketRateSheet<TenorKey>  — Deposit / IRS，key 為 tenor 字串如 "3M"
//   MarketRateSheet<NaiveDate> — Future，key 為到期日

pub struct MarketRateSheet<T: Maturity> {
    generator_name: String,
    generator_type: InterestRateGeneratorType,
    sheet:          HashMap<T, f64>,
}

impl<T: Maturity> MarketRateSheet<T> {
    pub fn new(generator_name: String, generator_type: InterestRateGeneratorType) -> Self {
        MarketRateSheet {
            generator_name,
            generator_type,
            sheet: HashMap::new(),
        }
    }

    pub fn add_quote(&mut self, key: T, value: f64) {
        self.sheet.insert(key, value);
    }

    pub fn get_quote(&self, key: &T) -> Option<&f64> {
        self.sheet.get(key)
    }

    pub fn generator_name(&self) -> &String {
        &self.generator_name
    }

    pub fn generator_type(&self) -> &InterestRateGeneratorType {
        &self.generator_type
    }

    /// 取得報價、apply 到 generator setter、產生 instrument。
    pub fn generate_instrument(
        &self,
        maturity:             &T,
        position:             Position,
        trade_date:           NaiveDate,
        generator_collection: &InterestRateInstrumentGeneratorCollection,
    ) -> Result<Arc<dyn SimpleInstrument>, MarketRateSheetError> {
        let market_rate = self
            .sheet
            .get(maturity)
            .ok_or_else(|| MarketRateSheetError::MaturityNotFound(maturity.to_string()))?;

        match &self.generator_type {
            InterestRateGeneratorType::Deposit => {
                let generator = generator_collection
                    .deposit_generator_manager
                    .get(&self.generator_name)?;

                generator
                    .leg_character_genrator()
                    .setter()
                    .set_fixed_rate(*market_rate);

                maturity.generate(position, trade_date, generator.as_ref())
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
                        setter.set_fixed_rate(*market_rate),
                    InterestRateSwapQuoteTarget::Spread =>
                        setter.set_spread(*market_rate),
                }

                maturity.generate(position, trade_date, generator.as_ref())
            }
        }
    }
}