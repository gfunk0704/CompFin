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
            RangeOfDates {start_date: d2, end_date: d1}
        } else {
            RangeOfDates {start_date: d1, end_date: d2}
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
            index: 0,
        }
    }

    pub fn to_vec(&self) -> Vec<NaiveDate> {
        let mut date_vec: Vec<NaiveDate> = Vec::new();
        for d in self.iter() {
            date_vec.push(d);
        }
        date_vec
     }
}

pub struct RangeOfDatesIterator<'a> {
    range_of_dates: &'a RangeOfDates,
    index: usize,
}

impl<'a> Iterator for RangeOfDatesIterator<'a> {
    type Item = NaiveDate;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.range_of_dates.len() {
            let result: Option<NaiveDate> = Some(*(&self.range_of_dates.start_date()) + Days::new(self.index as u64));
            self.index += 1;
            result
        } else {
            None
        }
    }
}