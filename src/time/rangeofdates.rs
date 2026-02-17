use chrono::{
    Days,
    NaiveDate
};

pub struct RangeOfDates {
    start_date: NaiveDate,
    end_date: NaiveDate
}

impl RangeOfDates {
    pub fn new(d1: NaiveDate, d2: NaiveDate) -> RangeOfDates {
        if d1 > d2 {
            RangeOfDates { start_date: d2, end_date: d1 }
        } else {
            RangeOfDates { start_date: d1, end_date: d2 }
        }
    }

    pub fn start_date(&self) -> NaiveDate {
        self.start_date
    }

    pub fn end_date(&self) -> NaiveDate {
        self.end_date
    }

    pub fn len(&self) -> usize {
        ((self.end_date - self.start_date).num_days() + 1) as usize
    }

    pub fn contain(&self, d: NaiveDate) -> bool {
        (d >= self.start_date) && (d <= self.end_date)
    }

    pub fn iter(&self) -> RangeOfDatesIterator {
        RangeOfDatesIterator {
            range_of_dates: self,
            // 變更：原本儲存 index（usize），改為直接儲存當前日期（NaiveDate）。
            // 好處：每次 next() 只需呼叫 succ_opt()，避免原本 start_date + Days::new(index) 的乘法運算。
            current: Some(self.start_date),
        }
    }

    /// # 變更說明
    /// 加入 `Vec::with_capacity(self.len())` 預先分配記憶體。
    /// 原本 `Vec::new()` 在多次 push 時可能觸發多次 realloc；
    /// 預先分配後整個迭代過程只有一次記憶體分配。
    pub fn to_vec(&self) -> Vec<NaiveDate> {
        let mut date_vec = Vec::with_capacity(self.len()); // 變更：Vec::new() → Vec::with_capacity(self.len())
        date_vec.extend(self.iter());                       // 變更：for 迴圈 push → extend
        date_vec
    }
}

/// # 變更說明
/// 原本儲存 `index: usize`，每次 next() 計算 `start_date + Days::new(index)` 。
/// 改為直接儲存 `current: Option<NaiveDate>`，每次 next() 只呼叫 `succ_opt()`，
/// 去除每步的整數乘法，對長範圍迭代有細微但穩定的效能優勢。
pub struct RangeOfDatesIterator<'a> {
    range_of_dates: &'a RangeOfDates,
    current: Option<NaiveDate>, // 變更：index: usize → current: Option<NaiveDate>
}

impl<'a> Iterator for RangeOfDatesIterator<'a> {
    type Item = NaiveDate;

    fn next(&mut self) -> Option<Self::Item> {
        let cur = self.current?;
        if cur <= self.range_of_dates.end_date() {
            // 移至下一天（只需 +1 天，不再計算 start_date + Days::new(index)）
            self.current = cur.succ_opt();
            Some(cur)
        } else {
            None
        }
    }
}
