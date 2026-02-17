use std::sync::Arc; // 變更：Rc → Arc

use chrono::{Days, NaiveDate};

use super::super::schedule::schedule::Schedule;

// ─────────────────────────────────────────────────────────────────────────────
// Traits：全部加入 Send + Sync supertrait
// ─────────────────────────────────────────────────────────────────────────────

/// # 變更說明
/// 加入 `Send + Sync`，使 `dyn DayCounterNumerator` 可放入 `Arc`。
pub trait DayCounterNumerator: Send + Sync {
    fn days_between(&self, d1: NaiveDate, d2: NaiveDate) -> f64;
}

/// # 變更說明
/// 加入 `Send + Sync`。
/// `year_fraction` 的 `numerator` 參數由 `&Rc<...>` 改為 `&Arc<...>`。
pub trait DayCounterDominator: Send + Sync {
    fn year_fraction(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        numerator: &Arc<dyn DayCounterNumerator>, // 變更：Rc → Arc
    ) -> f64;
}

/// # 變更說明
/// 加入 `Send + Sync`。
/// `generate` 回傳型別由 `Rc<...>` 改為 `Arc<...>`。
pub trait DayCounterNumeratorGenerator: Send + Sync {
    fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<Arc<dyn DayCounterNumerator>, DayCounterGenerationError>; // 變更：Rc → Arc
}

/// # 變更說明
/// 加入 `Send + Sync`。
/// `generate` 回傳型別由 `Rc<...>` 改為 `Arc<...>`。
pub trait DayCounterDominatorGenerator: Send + Sync {
    fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<Arc<dyn DayCounterDominator>, DayCounterGenerationError>; // 變更：Rc → Arc
}

// ─────────────────────────────────────────────────────────────────────────────
// DayCounterGenerationError
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum DayCounterGenerationError {
    ScheduleNotGiven,
    IrregularFrequencyForICMADominator,
}

/// # 變更說明
/// 實作 `std::fmt::Display`，取代原本的 inherent `to_string()` 方法。
/// `Display` 自動透過 blanket impl 提供 `to_string()`，原呼叫方不受影響。
impl std::fmt::Display for DayCounterGenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DayCounterGenerationError::ScheduleNotGiven => {
                write!(f, "Schedule not given for day counter generation")
            }
            DayCounterGenerationError::IrregularFrequencyForICMADominator => {
                write!(f, "Irregular frequency given for ICMA actual day count dominator generation")
            }
        }
    }
}

/// # 新增：實作 `std::error::Error`
/// 使 `DayCounterGenerationError` 可與 `?`、`Box<dyn Error>`、`anyhow` 等整合。
impl std::error::Error for DayCounterGenerationError {}

// ─────────────────────────────────────────────────────────────────────────────
// DayCounter
// ─────────────────────────────────────────────────────────────────────────────

/// # 變更說明
/// `numerator` 與 `dominator` 欄位由 `Rc<...>` 改為 `Arc<...>`。
/// `Arc` 為執行緒安全的引用計數，是 `DayCounter` 能跨執行緒共享的必要前提。
pub struct DayCounter {
    numerator: Arc<dyn DayCounterNumerator>, // 變更：Rc → Arc
    dominator: Arc<dyn DayCounterDominator>, // 變更：Rc → Arc
    shift_days1: Days,
    shift_days2: Days,
}

impl DayCounter {
    pub fn new(
        include_d1: bool,
        include_d2: bool,
        numerator: Arc<dyn DayCounterNumerator>, // 變更：Rc → Arc
        dominator: Arc<dyn DayCounterDominator>, // 變更：Rc → Arc
    ) -> DayCounter {
        DayCounter {
            numerator,
            dominator,
            shift_days1: if include_d1 { Days::new(1) } else { Days::new(0) },
            shift_days2: if include_d2 { Days::new(0) } else { Days::new(1) },
        }
    }

    pub fn include_d1(&self) -> bool {
        self.shift_days1 == Days::new(1)
    }

    pub fn include_d2(&self) -> bool {
        self.shift_days2 == Days::new(0)
    }

    pub fn year_fraction(&self, d1: NaiveDate, d2: NaiveDate) -> f64 {
        if d1 == d2 {
            0.0
        } else if d1 > d2 {
            let start_date = d2 + self.shift_days1;
            let end_date = d1 + self.shift_days2;
            -self.dominator.year_fraction(start_date, end_date, &self.numerator)
        } else {
            let start_date = d1 + self.shift_days1;
            let end_date = d2 + self.shift_days2;
            self.dominator.year_fraction(start_date, end_date, &self.numerator)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DayCounterGenerator
// ─────────────────────────────────────────────────────────────────────────────

/// # 變更說明
/// `numerator_generator` 與 `dominator_generator` 欄位由 `Rc<...>` 改為 `Arc<...>`。
/// 這使得 `DayCounterGenerator` 本身也是 `Send + Sync`，可放入 `FrozenManager`。
pub struct DayCounterGenerator {
    numerator_generator: Arc<dyn DayCounterNumeratorGenerator>, // 變更：Rc → Arc
    dominator_generator: Arc<dyn DayCounterDominatorGenerator>, // 變更：Rc → Arc
    include_d1: bool,
    include_d2: bool,
}

impl DayCounterGenerator {
    pub fn new(
        numerator_generator: Arc<dyn DayCounterNumeratorGenerator>, // 變更：Rc → Arc
        dominator_generator: Arc<dyn DayCounterDominatorGenerator>, // 變更：Rc → Arc
        include_d1: bool,
        include_d2: bool,
    ) -> DayCounterGenerator {
        DayCounterGenerator {
            numerator_generator,
            dominator_generator,
            include_d1,
            include_d2,
        }
    }

    pub fn generate(
        &self,
        schedule_opt: Option<&Schedule>,
    ) -> Result<DayCounter, DayCounterGenerationError> {
        let numerator = self.numerator_generator.generate(schedule_opt)?;
        let dominator = self.dominator_generator.generate(schedule_opt)?;
        Ok(DayCounter::new(self.include_d1, self.include_d2, numerator, dominator))
    }
}
