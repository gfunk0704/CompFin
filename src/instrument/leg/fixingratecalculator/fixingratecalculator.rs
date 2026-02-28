// ── fixingratecalculator.rs ──────────────────────────────────────────────────

use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::pricingcondition::PricingCondition;
use crate::time::schedule::schedule::Schedule;


pub trait FixingRateCalculator: Send + Sync {
    fn index(&self) -> &Arc<dyn InterestRateIndex + Send + Sync>;

    fn relative_dates(&self, i: usize) -> HashSet<NaiveDate>;

    fn fixing(
        &self,
        i: usize,
        forward_curve: &Arc<dyn InterestRateCurve>,
        pricing_condition: &PricingCondition,
    ) -> f64;

    /// Sensitivity 模式切換：強制使用 Standard Forward（逐日 ∏）。
    ///
    /// 定價引擎透過此介面切換，不需要知道底層是哪種 calculator：
    ///
    /// ```text
    /// // sensitivity 計算前
    /// leg.fixing_rate_calculator().set_standard_forward(true);
    ///
    /// // 評價模式還原
    /// leg.fixing_rate_calculator().set_standard_forward(false);
    /// ```
    ///
    /// # 回傳值
    /// 實際生效的值（若 calculator 不支援切換，或條件不滿足，永遠回傳 false）。
    ///
    /// # 各實作的行為
    ///
    /// | Calculator | enable=true | enable=false |
    /// |---|---|---|
    /// | `TermRateCalculator` | false（不支援） | false |
    /// | `DailyCompoundedRateCalculator` | 視 `arbitrage_free_applicable` | false |
    /// | `CompoundingRateIndexCalculator` | 視 `arbitrage_free_applicable` | false |
    fn set_standard_forward(&self, _enable: bool) -> bool {
        false   // 預設 no-op：不支援切換的 calculator 不需要覆寫
    }
}


pub trait FixingRateCalculatorGenerator {
    fn index(&self) -> &Arc<dyn InterestRateIndex + Send + Sync>;

    fn generate(&self, schedule: &Schedule) -> Arc<dyn FixingRateCalculator>;
}
