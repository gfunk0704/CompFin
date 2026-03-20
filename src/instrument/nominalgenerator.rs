use std::sync::{Arc, RwLock};

use serde::Deserialize;

use crate::interestrate::compounding::Compounding;
use crate::manager::manager::FrozenManager;
use crate::manager::managererror::ManagerError;
use crate::time::daycounter::daycounter::{DayCounter, DayCounterGenerator};
use crate::time::schedule::schedule::Schedule;


pub struct NominalSetter {
    initial_nominal: RwLock<f64>,
    rate: RwLock<f64>,
}

impl NominalSetter {
    pub fn new() -> Self {
        Self {
            initial_nominal: RwLock::new(1_000_000.0),
            rate: RwLock::new(0.0),
        }
    }

    pub fn initial_nominal(&self) -> f64 {
        *self.initial_nominal.read().unwrap()
    }

    pub fn set_initial_nominal(&self, v: f64) {
        *self.initial_nominal.write().unwrap() = v;
    }

    pub fn rate(&self) -> f64 {
        *self.rate.read().unwrap()
    }

    pub fn set_rate(&self, v: f64) {
        *self.rate.write().unwrap() = v;
    }
}


pub trait NominalGenerator: Send + Sync {
    fn setter(&self) -> &NominalSetter;

    // 回傳每個schedule period對應的名目本金，與schedule_periods()一對一對應
    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64>;
}


pub struct FixedNominalGenerator {
    setter: NominalSetter,
}

impl FixedNominalGenerator {
    pub fn new() -> Self {
        Self {
            setter: NominalSetter::new(),
        }
    }
}

impl NominalGenerator for FixedNominalGenerator {
    fn setter(&self) -> &NominalSetter {
        &self.setter
    }

    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64> {
        let initial_nominal = self.setter.initial_nominal();
        vec![initial_nominal; schedule.schedule_periods().len()]
    }
}


pub struct AccretingNominalGenerator {
    day_counter: DayCounter,
    compounding: Compounding,
    setter: NominalSetter,
}

impl AccretingNominalGenerator {
    pub fn new(day_counter: DayCounter, compounding: Compounding) -> Self {
        Self {
            day_counter,
            compounding,
            setter: NominalSetter::new(),
        }
    }

    pub fn day_counter(&self) -> &DayCounter {
        &self.day_counter
    }

    pub fn compounding(&self) -> &Compounding {
        &self.compounding
    }
}

impl NominalGenerator for AccretingNominalGenerator {
    fn setter(&self) -> &NominalSetter {
        &self.setter
    }

    fn generate_nominal(&self, schedule: &Schedule) -> Vec<f64> {
        let periods = schedule.schedule_periods();
        let rate = self.setter.rate();
        let mut current_nominal = self.setter.initial_nominal();

        let mut nominals = Vec::with_capacity(periods.len());
        nominals.push(current_nominal);

        // 最後一個period的名目本金由前一個period推算，不需要再複利
        for period in periods.split_last().unwrap().1 {
            let cp = period.calculation_period();
            let tau = self.day_counter.year_fraction(
                cp.start_date(),
                cp.end_date(),
            );
            current_nominal *= self.compounding.future_value(rate, tau);
            nominals.push(current_nominal);
        }

        nominals
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NominalGeneratorJsonProp / build_nominal_generator
// ─────────────────────────────────────────────────────────────────────────────
//
// 放在此模組的理由：
//   NominalGenerator 的 JSON 解析只依賴 DayCounterGenerator，
//   與利率商品、leg、market 等上層概念完全無關。
//   簽名直接反映真實依賴，Bond 等其他商品可以直接引用，
//   不需要帶入利率商品專屬的 supports 結構。
//
// JSON 範例（固定名目本金）：
//   { "type": "Fixed", "initial_nominal": 100000000.0 }
//
// JSON 範例（遞增名目本金）：
//   {
//     "type": "Accreting",
//     "initial_nominal": 100000000.0,
//     "rate": 0.03,
//     "day_counter_generator": "ACT365",
//     "compounding": "Annual"
//   }

/// 名目本金產生器的內嵌 JSON 定義。
///
/// 以 `type` 欄位區分，對應到 [`FixedNominalGenerator`] 或 [`AccretingNominalGenerator`]。
#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum NominalGeneratorJsonProp {
    /// 固定名目本金：每個 schedule period 的本金均相同。
    Fixed {
        initial_nominal: f64,
    },
    /// 遞增（複利累積）名目本金：本金隨每個 period 依利率複利成長。
    ///
    /// `day_counter_generator` 從呼叫端傳入的 `FrozenManager<DayCounterGenerator>` 查找。
    Accreting {
        initial_nominal: f64,
        rate: f64,
        day_counter_generator: String,
        compounding: Compounding,
    },
}

/// [`NominalGeneratorJsonProp`] 轉換為 `Arc<dyn NominalGenerator>`。
///
/// 簽名只取真正需要的 `dcg_manager`，不依賴任何上層的 supports 結構，
/// 讓 Bond 等非利率商品也能直接呼叫。
pub fn build_nominal_generator(
    prop: NominalGeneratorJsonProp,
    dcg_manager: &FrozenManager<DayCounterGenerator>,
) -> Result<Arc<dyn NominalGenerator>, ManagerError> {
    match prop {
        NominalGeneratorJsonProp::Fixed { initial_nominal } => {
            let nominal_gen = FixedNominalGenerator::new();
            nominal_gen.setter().set_initial_nominal(initial_nominal);
            Ok(Arc::new(nominal_gen))
        }
        NominalGeneratorJsonProp::Accreting { initial_nominal, rate, day_counter_generator, compounding } => {
            let dcg         = dcg_manager.get(&day_counter_generator)?;
            // generate(None)：AccretingNominalGenerator 不依賴具體 schedule，可用預設參數
            let day_counter = dcg.generate(None)?;
            let nominal_gen = AccretingNominalGenerator::new(day_counter, compounding);
            nominal_gen.setter().set_initial_nominal(initial_nominal);
            nominal_gen.setter().set_rate(rate);
            Ok(Arc::new(nominal_gen))
        }
    }
}
