
#[inline]
pub const fn is_leap (year: i32) -> bool {
    ((year % 4 == 0) && (year % 100!= 0)) || (year % 400 == 0)
}


pub const fn days_of_month (year: i32, month: u32) -> u32 {
    const NO_LEAP_EOM: [u32; 13] = [
        0, 31, 28, 31, 30, 
        31, 30, 31, 31, 30,
        31, 30, 31
    ];

    const LEAP_EOM: [u32; 13] = [
        0, 31, 29, 31, 30, 
        31, 30, 31, 31, 30,
        31, 30, 31
    ];
    
    if is_leap(year) {
        LEAP_EOM[month as usize]
    } else {
        NO_LEAP_EOM[month as usize]
    }
}

#[inline]
pub fn leap_years_before (year: i32) -> i32 {
    assert!(year > 0);
    let pre_year = year - 1;
    (pre_year / 4) - (pre_year / 100) + (pre_year / 400)
}

#[inline]
pub fn leap_years_between (start_year: i32, end_year: i32) -> i32 {
    assert!(end_year > start_year);
    leap_years_before(end_year) - leap_years_before(start_year + 1)
}

