use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::model::interestrate::interestratecurve::InterestRateCurve;
use crate::time::daycounter::daycounter::DayCounter;

/// Storage strategy for discount factor caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStrategy {
    /// Automatically choose based on data characteristics.
    /// - Dense (Vec): if span ≤ 2 years AND fill rate > 20%
    /// - Sparse (HashMap): otherwise
    Auto,

    /// Force sparse storage using HashMap.
    /// Best for: scattered dates (e.g., quarterly cashflows over many years)
    /// Memory: ~24 bytes per date
    /// Speed: ~15ns per lookup
    Sparse,

    /// Force dense storage using Vec.
    /// Best for: consecutive dates (e.g., daily SOFR compounding)
    /// Memory: 8 bytes per day in range
    /// Speed: ~7ns per lookup
    /// Requires: reference_date and max_days
    Dense {
        reference_date: NaiveDate,
        max_days: usize,
    },
}

impl Default for CacheStrategy {
    fn default() -> Self {
        CacheStrategy::Auto
    }
}

/// Internal storage for cached discount factors.
enum CacheStorage {
    Sparse(HashMap<NaiveDate, f64>),
    Dense {
        reference_date: NaiveDate,
        values: Vec<f64>, // NaN = not cached
    },
}

impl CacheStorage {
    #[inline]
    fn get(&self, date: NaiveDate) -> Option<f64> {
        match self {
            CacheStorage::Sparse(map) => map.get(&date).copied(),
            CacheStorage::Dense {
                reference_date,
                values,
            } => {
                let days = (date - *reference_date).num_days();
                if days >= 0 && (days as usize) < values.len() {
                    let val = values[days as usize];
                    if !val.is_nan() {
                        return Some(val);
                    }
                }
                None
            }
        }
    }
}

pub struct PrecomputedDiscountCurve {
    reference_curve: Arc<dyn InterestRateCurve>,
    storage: CacheStorage,
}

impl PrecomputedDiscountCurve {
    /// Create a precomputed discount curve with specified caching strategy.
    ///
    /// # Arguments
    /// * `reference_curve` - The base discount curve to cache
    /// * `dates` - Dates to precompute and cache
    /// * `strategy` - Caching strategy (Auto, Sparse, or Dense)
    ///
    /// # Examples
    ///
    /// ```
    /// // Auto mode: let the system decide
    /// let curve = PrecomputedDiscountCurve::new(
    ///     base_curve,
    ///     &dates,
    ///     CacheStrategy::Auto,
    /// );
    ///
    /// // Force sparse (HashMap) for IRS
    /// let curve = PrecomputedDiscountCurve::new(
    ///     base_curve,
    ///     &quarterly_dates,
    ///     CacheStrategy::Sparse,
    /// );
    ///
    /// // Force dense (Vec) for SOFR compounding
    /// let curve = PrecomputedDiscountCurve::new(
    ///     base_curve,
    ///     &daily_dates,
    ///     CacheStrategy::Dense {
    ///         reference_date: today,
    ///         max_days: 365,
    ///     },
    /// );
    /// ```
    pub fn new(
        reference_curve: Arc<dyn InterestRateCurve>,
        dates: &[NaiveDate],
        strategy: CacheStrategy,
    ) -> Self {
        let storage = match strategy {
            CacheStrategy::Auto => Self::create_auto_storage(&reference_curve, dates),
            CacheStrategy::Sparse => Self::create_sparse_storage(&reference_curve, dates),
            CacheStrategy::Dense {
                reference_date,
                max_days,
            } => Self::create_dense_storage(&reference_curve, reference_date, max_days, dates),
        };

        PrecomputedDiscountCurve {
            reference_curve,
            storage,
        }
    }

    /// Create storage with automatic strategy selection.
    fn create_auto_storage(
        reference_curve: &Arc<dyn InterestRateCurve>,
        dates: &[NaiveDate],
    ) -> CacheStorage {
        if dates.is_empty() {
            return CacheStorage::Sparse(HashMap::new());
        }

        let min_date = *dates.iter().min().unwrap();
        let max_date = *dates.iter().max().unwrap();
        let span_days = (max_date - min_date).num_days();
        let num_dates = dates.len();

        // Auto decision: Dense if fill rate > 20%
        let use_dense = (num_dates as f64 / (span_days + 1) as f64) > 0.2;

        if use_dense {
            Self::create_dense_storage(
                reference_curve,
                min_date,
                (span_days + 1) as usize,
                dates,
            )
        } else {
            Self::create_sparse_storage(reference_curve, dates)
        }
    }

    /// Create sparse (HashMap) storage.
    fn create_sparse_storage(
        reference_curve: &Arc<dyn InterestRateCurve>,
        dates: &[NaiveDate],
    ) -> CacheStorage {
        let mut map = HashMap::with_capacity(dates.len());

        for &date in dates {
            map.insert(date, reference_curve.discount(date));
        }

        CacheStorage::Sparse(map)
    }

    /// Create dense (Vec) storage.
    fn create_dense_storage(
        reference_curve: &Arc<dyn InterestRateCurve>,
        reference_date: NaiveDate,
        max_days: usize,
        dates: &[NaiveDate],
    ) -> CacheStorage {
        let mut values = vec![f64::NAN; max_days];

        for &date in dates {
            let days = (date - reference_date).num_days();
            if days >= 0 && (days as usize) < max_days {
                values[days as usize] = reference_curve.discount(date);
            }
        }

        CacheStorage::Dense {
            reference_date,
            values,
        }
    }

    /// Get information about the storage type being used.
    pub fn storage_info(&self) -> &str {
        match &self.storage {
            CacheStorage::Sparse(_) => "Sparse (HashMap)",
            CacheStorage::Dense { .. } => "Dense (Vec)",
        }
    }

    /// Get the approximate memory footprint in bytes.
    pub fn memory_bytes(&self) -> usize {
        match &self.storage {
            CacheStorage::Sparse(map) => {
                // HashMap: ~24 bytes per entry + overhead
                map.len() * 24 + 1000
            }
            CacheStorage::Dense { values, .. } => {
                // Vec: 8 bytes per element
                values.len() * 8
            }
        }
    }

    /// Get the number of cached entries.
    pub fn cached_count(&self) -> usize {
        match &self.storage {
            CacheStorage::Sparse(map) => map.len(),
            CacheStorage::Dense { values, .. } => {
                values.iter().filter(|v| !v.is_nan()).count()
            }
        }
    }

    pub fn reference_curve(&self) -> &Arc<dyn InterestRateCurve> {
        &self.reference_curve
    }
}

impl InterestRateCurve for PrecomputedDiscountCurve {
    fn day_counter(&self) -> Arc<DayCounter> {
        self.reference_curve.day_counter()
    }

    fn reference_date(&self) -> NaiveDate {
        self.reference_curve.reference_date()
    }

    #[inline]
    fn discount(&self, d: NaiveDate) -> f64 {
        self.storage
            .get(d)
            .unwrap_or_else(|| self.reference_curve.discount(d))
    }
}