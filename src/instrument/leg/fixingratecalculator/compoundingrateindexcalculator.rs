// ── compoundingrateindexcalculator.rs ────────────────────────────────────────
//
// CompoundingRateIndexCalculator 是 CompoundingRateIndex 在 leg 層的包裝。
//
// # 與 DailyCompoundedRateCalculator 的差異
//
// | | DailyCompoundedRateCalculator | CompoundingRateIndexCalculator |
// |---|---|---|
// | 底層 index | TermRateIndex（1D tenor） | CompoundingRateIndex |
// | 逐日展開 | Calculator 自己做 | Index 內部做 |
// | 切換入口 | `self.set_apply_arbitrage_free()` | `self.index.set_use_arbitrage_free()` |
// | Stub 支援 | 無（1D index 無 stub） | 天然支援（逐日計算） |
//
// # set_standard_forward 的實作
//
// 直接委託給 CompoundingRateIndex::set_use_arbitrage_free(!enable)。
// 回傳值語意與 DailyCompoundedRateCalculator 相同：
//   - enable=true 且 applicable → true（成功切換為 Standard Forward）
//   - enable=true 且不 applicable → false（本來就是 Standard Forward，無需切換）
//   - enable=false → false（已在評價模式）

use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;

use super::fixingratecalculator::{FixingRateCalculator, FixingRateCalculatorGenerator};
use crate::interestrate::index::compoundingrateindex::CompoundingRateIndex;
use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::schedule::schedule::Schedule;
use crate::time::schedule::scheduleperiod::CalculationPeriod;


pub struct CompoundingRateIndexCalculator {
    /// typed Arc，供 set_standard_forward 使用（需要呼叫 CompoundingRateIndex 特有方法）
    compounding_index: Arc<CompoundingRateIndex>,
    /// Arc<dyn InterestRateIndex + Send + Sync>，供 FixingRateCalculator::index() 回傳
    /// 建構時從 compounding_index 轉型而來，避免每次呼叫都重新建立
    index_dyn: Arc<dyn InterestRateIndex + Send + Sync>,
    periods: Vec<CalculationPeriod>,
}

impl CompoundingRateIndexCalculator {
    pub fn new(index: Arc<CompoundingRateIndex>, schedule: &Schedule) -> Self {
        let periods = schedule
            .schedule_periods()
            .iter()
            .map(|sp| sp.calculation_period())
            .collect();
        // Arc<CompoundingRateIndex> → Arc<dyn InterestRateIndex + Send + Sync>
        let index_dyn: Arc<dyn InterestRateIndex + Send + Sync> = index.clone();
        Self { compounding_index: index, index_dyn, periods }
    }
}

impl FixingRateCalculator for CompoundingRateIndexCalculator {
    fn index(&self) -> &Arc<dyn InterestRateIndex + Send + Sync> {
        &self.index_dyn
    }

    fn relative_dates(&self, i: usize) -> HashSet<NaiveDate> {
        self.compounding_index.relative_dates_for_period(&self.periods[i])
    }

    fn fixing(
        &self,
        i: usize,
        forward_curve: &Arc<dyn InterestRateCurve>,
        pricing_condition: &PricingCondition,
    ) -> f64 {
        self.compounding_index
            .fixing_rate_for_period(&self.periods[i], Some(forward_curve), pricing_condition)
            .unwrap_or(0.0)
    }

    /// 切換為 Standard Forward 模式（sensitivity 計算用）。
    ///
    /// 委託給 CompoundingRateIndex::set_use_arbitrage_free(!enable)：
    /// - Standard Forward = arbitrage-free 關閉
    /// - 若條件不滿足（lookback_days != 0 或 Arrear），切換無效，回傳 false
    ///
    /// # 回傳值
    /// 「是否實際發生了模式切換」：
    /// - enable=true  且 applicable=true  → true（成功從 AF 切換到 SF）
    /// - enable=true  且 applicable=false → false（本來就是 SF，無需切換）
    /// - enable=false                    → false（還原評價模式，非切換）
    fn set_standard_forward(&self, enable: bool) -> bool {
        self.compounding_index.set_use_arbitrage_free(!enable);
        enable && self.compounding_index.arbitrage_free_applicable()
    }
}


pub struct CompoundingRateIndexCalculatorGenerator {
    compounding_index: Arc<CompoundingRateIndex>,
    index_dyn: Arc<dyn InterestRateIndex + Send + Sync>,
}

impl CompoundingRateIndexCalculatorGenerator {
    pub fn new(index: Arc<CompoundingRateIndex>) -> Self {
        let index_dyn: Arc<dyn InterestRateIndex + Send + Sync> = index.clone();
        Self { compounding_index: index, index_dyn }
    }
}

impl FixingRateCalculatorGenerator for CompoundingRateIndexCalculatorGenerator {
    fn index(&self) -> &Arc<dyn InterestRateIndex + Send + Sync> {
        &self.index_dyn
    }

    fn generate(&self, schedule: &Schedule) -> Arc<dyn FixingRateCalculator> {
        Arc::new(CompoundingRateIndexCalculator::new(self.compounding_index.clone(), schedule))
    }
}
