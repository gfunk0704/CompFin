use std::collections::HashSet;
use std::sync::Arc; // 變更：Rc → Arc

use chrono::NaiveDate;

use crate::time::calendar::holidaycalendar::HolidayCalendar;

/// Combines two calendars using logical operations (union or intersection).
///
/// # Union
/// A date is a holiday if it's a holiday in EITHER calendar.
/// Use case: "Holiday if it's a holiday in US OR UK"
///
/// # Intersection
/// A date is a holiday if it's a holiday in BOTH calendars.
/// Use case: "Holiday only if it's a holiday in BOTH markets"
///
/// # 變更說明
/// - 所有 `Rc<dyn HolidayCalendar>` 改為 `Arc<dyn HolidayCalendar>`。
///   `HolidayCalendar` 已加入 `Send + Sync` supertrait，故 `dyn HolidayCalendar`
///   可直接放入 `Arc` 而無需額外標注。
pub struct JointCalendar {
    c1: Arc<dyn HolidayCalendar>, // 變更：Rc → Arc
    c2: Arc<dyn HolidayCalendar>, // 變更：Rc → Arc
    logical_operator: fn(bool, bool) -> bool
}

impl JointCalendar {
    /// Creates a union of two calendars.
    /// A date is a holiday if it's a holiday in c1 OR c2.
    pub fn union(c1: Arc<dyn HolidayCalendar>, c2: Arc<dyn HolidayCalendar>) -> JointCalendar {
        JointCalendar {
            c1,
            c2,
            logical_operator: |b1, b2| b1 || b2
        }
    }

    /// Creates an intersection of two calendars.
    /// A date is a holiday if it's a holiday in c1 AND c2.
    pub fn intersection(c1: Arc<dyn HolidayCalendar>, c2: Arc<dyn HolidayCalendar>) -> JointCalendar {
        JointCalendar {
            c1,
            c2,
            logical_operator: |b1, b2| b1 && b2
        }
    }

    /// Returns true if this is a union calendar
    pub fn is_union(&self) -> bool {
        (self.logical_operator)(true, false)
    }

    /// Returns true if this is an intersection calendar
    pub fn is_intersection(&self) -> bool {
        !self.is_union()
    }

    /// Returns a reference to the first calendar
    pub fn c1(&self) -> &Arc<dyn HolidayCalendar> {
        &self.c1
    }

    /// Returns a reference to the second calendar
    pub fn c2(&self) -> &Arc<dyn HolidayCalendar> {
        &self.c2
    }

    /// Returns a cloned Arc to the first calendar (for legacy compatibility)
    pub fn c1_cloned(&self) -> Arc<dyn HolidayCalendar> {
        Arc::clone(&self.c1)
    }

    /// Returns a cloned Arc to the second calendar (for legacy compatibility)
    pub fn c2_cloned(&self) -> Arc<dyn HolidayCalendar> {
        Arc::clone(&self.c2)
    }
}

impl HolidayCalendar for JointCalendar {
    #[inline]
    fn is_holiday(&self, d: NaiveDate) -> bool {
        (self.logical_operator)(self.c1.is_holiday(d), self.c2.is_holiday(d))
    }

    fn get_holiday_set(&self, year: i32) -> HashSet<NaiveDate> {
        let s1 = self.c1.get_holiday_set(year);
        let s2 = self.c2.get_holiday_set(year);

        if self.is_union() {
            s1.union(&s2).copied().collect()
        } else {
            s1.intersection(&s2).copied().collect()
        }
    }
}
