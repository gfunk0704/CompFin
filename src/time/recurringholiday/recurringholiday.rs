use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};

/// # 變更說明
/// - 加入 `Send + Sync` supertrait，使 `dyn RecurringHoliday` 可安全用於 Arc 與多執行緒環境。
pub trait RecurringHoliday: Send + Sync {

    fn get_holiday(&self, year: i32) -> HashSet<NaiveDate>;

    fn is_holiday(&self, d: &NaiveDate) -> bool {
        let holiday_set = self.get_holiday(d.year());
        holiday_set.contains(d)
    }
}
