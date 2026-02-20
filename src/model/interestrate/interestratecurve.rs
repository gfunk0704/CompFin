use std::sync::Arc;

use chrono::NaiveDate;

use crate::time::daycounter::daycounter::DayCounter;

/// 利率曲線 trait。
///
/// # 設計說明：移除 ObjectWithUUID
/// 原本繼承 `ObjectWithUUID` 是為了讓 `CachedInterestRateIndex` 透過 UUID
/// 判斷 forward curve 是否更換，進而決定要不要清 cache。
///
/// 但 UUID 方案的問題在於：
/// 1. 每個 curve struct 都需要持有一個 `Uuid` field，增加樣板程式碼。
/// 2. 語意上想表達的其實是「pointer identity」（是否為同一個物件），
///    Rust 中可直接用 `Arc::as_ptr() as usize` 表達，不需要額外的 UUID。
///
/// 因此 cache key 改為 `Arc::as_ptr() as usize`，`ObjectWithUUID` 不再是 supertrait。
/// 舊版 `ObjectWithUUID` trait 保留在 `objectwithuuid.rs`，供其他有需要的地方使用。
///
/// # 多執行緒安全
/// `Send + Sync` 是 supertrait，使 `Arc<dyn InterestRateCurve>` 可跨執行緒傳遞。
pub trait InterestRateCurve: Send + Sync {
    fn day_counter(&self) -> Arc<DayCounter>;

    fn reference_date(&self) -> NaiveDate;

    fn discount(&self, d: NaiveDate) -> f64;

    fn zero_rate(&self, d: NaiveDate) -> f64 {
        let t = self.day_counter().year_fraction(self.reference_date(), d);
        -self.discount(d).ln() / t
    }
}
