use chrono::NaiveDate;


// ─────────────────────────────────────────────────────────────────────────────
// CalculationPeriod
// ─────────────────────────────────────────────────────────────────────────────

/// 一個 calculation period，攜帶實際區間與對應的標準 regular 區間。
///
/// # Stub 識別
///
/// `StubAdjuster` 處理完之後，stub 的資訊本來會消失（只剩截短後的日期）。
/// 為了讓 `TermRateIndex` 在計算 stub fixing 時能知道對應的 regular period 長度，
/// 這裡保留兩組日期：
///
/// | 欄位                | 非 stub            | Stub（例如 retain）             |
/// |---------------------|--------------------|---------------------------------|
/// | `start_date`        | 實際起始日         | 實際起始日                      |
/// | `end_date`          | 實際結束日         | 截短後的實際結束日（= maturity）|
/// | `regular_start_date`| 同 `start_date`    | 自然 tenor 的起始日             |
/// | `regular_end_date`  | 同 `end_date`      | 自然 tenor 的結束日（未截短）   |
///
/// # 範例
///
/// 18Y IRS，backward generation，3M tenor，maturity = 2024-07-20：
///
/// ```text
/// // 最後一個 period（已截短為 short back stub）
/// CalculationPeriod::stub(
///     NaiveDate::from_ymd(2024, 4, 15),   // start_date
///     NaiveDate::from_ymd(2024, 7, 20),   // end_date（截短至 maturity）
///     NaiveDate::from_ymd(2024, 4, 15),   // regular_start_date
///     NaiveDate::from_ymd(2024, 7, 15),   // regular_end_date（自然 3M 結束）
/// )
/// // is_stub() == true
/// ```
#[derive(Clone, Copy)]
pub struct CalculationPeriod {
    start_date: NaiveDate,
    end_date: NaiveDate,
    regular_start_date: NaiveDate,
    regular_end_date: NaiveDate,
}

impl CalculationPeriod {
    /// 建立正常（非 stub）的 calculation period。
    /// `regular_start/end` 自動等於 `start/end`。
    pub fn regular(start_date: NaiveDate, end_date: NaiveDate) -> Self {
        Self {
            start_date,
            end_date,
            regular_start_date: start_date,
            regular_end_date: end_date,
        }
    }

    /// 建立 stub calculation period，明確傳入對應的 regular period 範圍。
    ///
    /// `regular_start_date` / `regular_end_date` 代表若無 stub 截斷時，
    /// 此 period 在自然 tenor 下的完整範圍（供 stub fixing 計算使用）。
    pub fn stub(
        start_date: NaiveDate,
        end_date: NaiveDate,
        regular_start_date: NaiveDate,
        regular_end_date: NaiveDate,
    ) -> Self {
        Self {
            start_date,
            end_date,
            regular_start_date,
            regular_end_date,
        }
    }

    pub fn start_date(&self) -> NaiveDate {
        self.start_date
    }

    pub fn end_date(&self) -> NaiveDate {
        self.end_date
    }

    pub fn regular_start_date(&self) -> NaiveDate {
        self.regular_start_date
    }

    pub fn regular_end_date(&self) -> NaiveDate {
        self.regular_end_date
    }

    /// 此 period 是否為 stub（實際區間與 regular 區間不完全相同）。
    pub fn is_stub(&self) -> bool {
        self.start_date != self.regular_start_date
            || self.end_date != self.regular_end_date
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// SchedulePeriod
// ─────────────────────────────────────────────────────────────────────────────

pub struct SchedulePeriod {
    fixing_date: NaiveDate,
    calculation_period: CalculationPeriod,
    payment_date: NaiveDate,
}

impl SchedulePeriod {
    pub fn new(
        fixing_date: NaiveDate,
        calculation_period: CalculationPeriod,
        payment_date: NaiveDate,
    ) -> SchedulePeriod {
        SchedulePeriod {
            fixing_date,
            calculation_period,
            payment_date,
        }
    }

    pub fn fixing_date(&self) -> NaiveDate {
        self.fixing_date
    }

    pub fn calculation_period(&self) -> CalculationPeriod {
        self.calculation_period
    }

    pub fn payment_date(&self) -> NaiveDate {
        self.payment_date
    }
}
