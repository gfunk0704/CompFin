use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::market::market::Market;
use crate::time::businessdayadjuster::BusinessDayAdjuster;
use crate::time::period::{Period, TimeUnit};

/// Option expiry / delivery 的計算方向（Clark §1.5）
///
/// * `ExpiryToDelivery`：先確定 expiry，delivery 從 expiry 往後推
///   → 適用 Days / Weeks tenor（例如 FX 1W option）
/// * `DeliveryToExpiry`：先確定 delivery（spot + tenor），expiry 從 delivery 往前推
///   → 適用 Months / Years tenor（例如 FX 1M、1Y option）
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExpiryRule {
    ExpiryToDelivery,
    DeliveryToExpiry,
}

/// Option expiry / delivery date 的產生器
///
/// 短天期（`short_term_time_unit` 內的 TimeUnit）與長天期可分別設定不同的
/// `ExpiryRule` 與 `BusinessDayAdjuster`，以對應各資產類別的市場慣例。
pub struct OptionDateGenerator {
    market: Arc<dyn Market>,
    short_term_expiry_rule: ExpiryRule,
    short_term_adjuster: BusinessDayAdjuster,
    long_term_expiry_rule: ExpiryRule,
    long_term_adjuster: BusinessDayAdjuster,
    /// 判定為短天期的 TimeUnit 集合（例如 {Days, Weeks}）
    short_term_time_unit: HashSet<TimeUnit>,
}

// ─── 私有輔助函式 ────────────────────────────────────────────────────────────

/// ExpiryToDelivery：expiry = horizon + tenor（expiry_calendar）
fn expiry_from_horizon(
    horizon: NaiveDate,
    tenor: Period,
    market: &Arc<dyn Market>,
    adjuster: &BusinessDayAdjuster,
) -> NaiveDate {
    adjuster.from_tenor_to_date(horizon, tenor, &market.expiry_calendar())
}

/// DeliveryToExpiry：delivery = spot + tenor（settlement_calendar）
fn delivery_from_spot(
    horizon: NaiveDate,
    tenor: Period,
    market: &Arc<dyn Market>,
    adjuster: &BusinessDayAdjuster,
) -> NaiveDate {
    let spot = market.settlement_date(horizon);
    adjuster.from_tenor_to_date(spot, tenor, &market.settlement_calendar())
}

/// DeliveryToExpiry：expiry = delivery - settlement_days BD（expiry_calendar）
fn expiry_from_delivery(
    horizon: NaiveDate,
    tenor: Period,
    market: &Arc<dyn Market>,
    adjuster: &BusinessDayAdjuster,
) -> NaiveDate {
    let delivery = delivery_from_spot(horizon, tenor, market, adjuster);
    market.expiry_calendar()
        .shift_n_business_day(delivery, -(market.settlement_days() as i32))
}

/// ExpiryToDelivery：delivery = expiry + settlement_days BD（settlement_calendar）
fn delivery_from_expiry(
    horizon: NaiveDate,
    tenor: Period,
    market: &Arc<dyn Market>,
    adjuster: &BusinessDayAdjuster,
) -> NaiveDate {
    let expiry = expiry_from_horizon(horizon, tenor, market, adjuster);
    market.settlement_calendar()
        .shift_n_business_day(expiry, market.settlement_days() as i32)
}

// ─── impl ────────────────────────────────────────────────────────────────────

impl OptionDateGenerator {
    pub fn new(
        market: Arc<dyn Market>,
        short_term_expiry_rule: ExpiryRule,
        short_term_adjuster: BusinessDayAdjuster,
        long_term_expiry_rule: ExpiryRule,
        long_term_adjuster: BusinessDayAdjuster,
        short_term_time_unit: HashSet<TimeUnit>,
    ) -> Self {
        Self {
            market,
            short_term_expiry_rule,
            short_term_adjuster,
            long_term_expiry_rule,
            long_term_adjuster,
            short_term_time_unit,
        }
    }

    fn is_short_term(&self, tenor: Period) -> bool {
        self.short_term_time_unit.contains(&tenor.unit())
    }

    pub fn generate_expiry(&self, horizon: NaiveDate, tenor: Period) -> NaiveDate {
        if self.is_short_term(tenor) {
            match self.short_term_expiry_rule {
                ExpiryRule::ExpiryToDelivery =>
                    expiry_from_horizon(horizon, tenor, &self.market, &self.short_term_adjuster),
                ExpiryRule::DeliveryToExpiry =>
                    expiry_from_delivery(horizon, tenor, &self.market, &self.short_term_adjuster),
            }
        } else {
            match self.long_term_expiry_rule {
                ExpiryRule::ExpiryToDelivery =>
                    expiry_from_horizon(horizon, tenor, &self.market, &self.long_term_adjuster),
                ExpiryRule::DeliveryToExpiry =>
                    expiry_from_delivery(horizon, tenor, &self.market, &self.long_term_adjuster),
            }
        }
    }

    pub fn generate_delivery(&self, horizon: NaiveDate, tenor: Period) -> NaiveDate {
        if self.is_short_term(tenor) {
            match self.short_term_expiry_rule {
                ExpiryRule::ExpiryToDelivery =>
                    delivery_from_expiry(horizon, tenor, &self.market, &self.short_term_adjuster),
                ExpiryRule::DeliveryToExpiry =>
                    delivery_from_spot(horizon, tenor, &self.market, &self.short_term_adjuster),
            }
        } else {
            match self.long_term_expiry_rule {
                ExpiryRule::ExpiryToDelivery =>
                    delivery_from_expiry(horizon, tenor, &self.market, &self.long_term_adjuster),
                ExpiryRule::DeliveryToExpiry =>
                    delivery_from_spot(horizon, tenor, &self.market, &self.long_term_adjuster),
            }
        }
    }
}