use chrono::NaiveDate;
use serde::Deserialize;

use super::scheduleperiod::CalculationPeriod;


// ─────────────────────────────────────────────────────────────────────────────
// stub adjuster 函式
// ─────────────────────────────────────────────────────────────────────────────
//
// 修改重點：
//   retain_last / retain_first  → 建立 CalculationPeriod::stub(...)
//     實際日期 = 截短後的範圍
//     regular  = 自然 tenor 的完整範圍（截短前）
//
//   combine_last / combine_first → 建立 CalculationPeriod::regular(...)
//     合併後的 period 視為一個特殊的「regular」長 period，
//     不做 stub fixing 特殊處理（兩個 period 合併，差距極小時才觸發）
//
//   extend / remove → 不產生新的 period，不受影響

fn extend(
    _forward: bool,
    _last_date: NaiveDate,
    _get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
    calculation_periods: Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    calculation_periods
}

fn remove(
    forward: bool,
    last_date: NaiveDate,
    get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
    calculation_periods: Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    let has_stub: bool = get_period_last_date(&calculation_periods) != last_date;
    if has_stub {
        if forward {
            calculation_periods[..(calculation_periods.len() - 1)].to_vec()
        } else {
            calculation_periods[1..].to_vec()
        }
    } else {
        calculation_periods
    }
}

/// 截短最後一個 period 的 end_date，保留 regular_end_date 為截短前的自然 end。
fn retain_last(
    last_date: NaiveDate,
    calculation_periods: &Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    let last = calculation_periods.last().unwrap();
    let mut adjusted = calculation_periods[..(calculation_periods.len() - 1)].to_vec();
    // stub：start 不變，end 截短至 last_date；regular_end 保留自然 tenor 的結束日
    adjusted.push(CalculationPeriod::stub(
        last.start_date(),
        last_date,
        last.regular_start_date(),
        last.regular_end_date(),
    ));
    adjusted
}

/// 延後第一個 period 的 start_date，保留 regular_start_date 為延後前的自然 start。
fn retain_first(
    last_date: NaiveDate,
    calculation_periods: &Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    let first = calculation_periods.first().unwrap();
    let mut adjusted = calculation_periods[1..].to_vec();
    // stub：start 延後至 last_date（front stub），end 不變；regular_start 保留自然 tenor 的起始日
    adjusted.insert(0, CalculationPeriod::stub(
        last_date,
        first.end_date(),
        first.regular_start_date(),
        first.regular_end_date(),
    ));
    adjusted
}

fn retain(
    forward: bool,
    last_date: NaiveDate,
    get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
    calculation_periods: Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    let has_stub: bool = get_period_last_date(&calculation_periods) != last_date;
    if has_stub {
        if forward {
            retain_last(last_date, &calculation_periods)
        } else {
            retain_first(last_date, &calculation_periods)
        }
    } else {
        calculation_periods
    }
}

/// 合併最後兩個 period（SmartCombine 在 stub 極短時使用）。
///
/// 合併後的 period 以 CalculationPeriod::regular 建立，
/// 表示它是一個完整的長 period，不做 stub fixing 特殊處理。
fn combine_last(
    last_date: NaiveDate,
    calculation_periods: &Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    let last     = calculation_periods.len() - 1;
    let penult   = last - 1;
    let mut adjusted = calculation_periods[..penult].to_vec();
    // 合併後視為 regular（非 stub）
    adjusted.push(CalculationPeriod::regular(
        calculation_periods[penult].start_date(),
        last_date,
    ));
    adjusted
}

/// 合併最前兩個 period。
fn combine_first(
    last_date: NaiveDate,
    calculation_periods: &Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    let mut adjusted = calculation_periods[2..].to_vec();
    adjusted.insert(0, CalculationPeriod::regular(
        last_date,
        calculation_periods[1].end_date(),
    ));
    adjusted
}

fn combine(
    forward: bool,
    last_date: NaiveDate,
    get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
    calculation_periods: Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    if calculation_periods.len() == 1 {
        return retain(forward, last_date, get_period_last_date, calculation_periods);
    }

    let has_stub: bool = get_period_last_date(&calculation_periods) != last_date;
    if has_stub {
        if forward {
            combine_last(last_date, &calculation_periods)
        } else {
            combine_first(last_date, &calculation_periods)
        }
    } else {
        calculation_periods
    }
}

fn smart_combine(
    forward: bool,
    last_date: NaiveDate,
    get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
    calculation_periods: Vec<CalculationPeriod>,
) -> Vec<CalculationPeriod> {
    if calculation_periods.len() == 1 {
        return retain(forward, last_date, get_period_last_date, calculation_periods);
    }

    if get_period_last_date(&calculation_periods) != last_date {
        if forward {
            let last_period = calculation_periods.last().unwrap();
            if (last_date - last_period.start_date()).num_days() < 7 {
                combine_last(last_date, &calculation_periods)
            } else {
                retain_last(last_date, &calculation_periods)
            }
        } else {
            let first_period = calculation_periods.first().unwrap();
            if (first_period.end_date() - last_date).num_days() < 7 {
                combine_first(last_date, &calculation_periods)
            } else {
                retain_first(last_date, &calculation_periods)
            }
        }
    } else {
        calculation_periods
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// StubConvention / StubAdjuster
// ─────────────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Eq, Clone, Copy, Deserialize)]
pub enum StubConvention {
    Extend,
    Remove,     // 修正原本的拼字錯誤（Reomve）
    Retain,
    Combine,
    SmartCombine,
}

#[derive(Clone, Copy)]
pub struct StubAdjuster {
    convention: StubConvention,
    forward: bool,
    get_period_last_date: fn(&Vec<CalculationPeriod>) -> NaiveDate,
    adjust_impl: fn(
        bool,
        NaiveDate,
        fn(&Vec<CalculationPeriod>) -> NaiveDate,
        Vec<CalculationPeriod>,
    ) -> Vec<CalculationPeriod>,
}

impl StubAdjuster {
    pub fn new(convention: StubConvention, forward: bool) -> StubAdjuster {
        let adjust_impl = match convention {
            StubConvention::Combine      => combine,
            StubConvention::Extend       => extend,
            StubConvention::Remove       => remove,
            StubConvention::Retain       => retain,
            StubConvention::SmartCombine => smart_combine,
        };

        let get_period_last_date = if forward {
            |p: &Vec<CalculationPeriod>| p.last().unwrap().end_date()
        } else {
            |p: &Vec<CalculationPeriod>| p.first().unwrap().start_date()
        };

        StubAdjuster {
            convention,
            forward,
            get_period_last_date,
            adjust_impl,
        }
    }

    pub fn convention(&self) -> StubConvention {
        self.convention
    }

    pub fn forward(&self) -> bool {
        self.forward
    }

    pub fn adjust(
        &self,
        last_date: NaiveDate,
        calculation_periods: Vec<CalculationPeriod>,
    ) -> Vec<CalculationPeriod> {
        (self.adjust_impl)(
            self.forward,
            last_date,
            self.get_period_last_date,
            calculation_periods,
        )
    }
}
