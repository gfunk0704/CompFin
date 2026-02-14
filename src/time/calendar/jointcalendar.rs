use std::collections::HashSet;
use std::rc::Rc;

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
pub struct JointCalendar {
    c1: Rc<dyn HolidayCalendar>,
    c2: Rc<dyn HolidayCalendar>,
    logical_operator: fn(bool, bool) -> bool
}

impl JointCalendar {
    /// Creates a union of two calendars.
    /// A date is a holiday if it's a holiday in c1 OR c2.
    pub fn union(c1: Rc<dyn HolidayCalendar>, c2: Rc<dyn HolidayCalendar>) -> JointCalendar {
        JointCalendar {
            c1, 
            c2, 
            logical_operator: |b1, b2| b1 || b2
        }
    }

    /// Creates an intersection of two calendars.
    /// A date is a holiday if it's a holiday in c1 AND c2.
    pub fn intersection(c1: Rc<dyn HolidayCalendar>, c2: Rc<dyn HolidayCalendar>) -> JointCalendar {
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
    pub fn c1(&self) -> &Rc<dyn HolidayCalendar> {
        &self.c1
    }

    /// Returns a reference to the second calendar
    pub fn c2(&self) -> &Rc<dyn HolidayCalendar> {
        &self.c2
    }

    /// Returns a cloned Rc to the first calendar (for legacy compatibility)
    pub fn c1_cloned(&self) -> Rc<dyn HolidayCalendar> {
        Rc::clone(&self.c1)
    }

    /// Returns a cloned Rc to the second calendar (for legacy compatibility)
    pub fn c2_cloned(&self) -> Rc<dyn HolidayCalendar> {
        Rc::clone(&self.c2)
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

