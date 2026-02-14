// ── cache_backend.rs ────────────────────────────────────────────────────────

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::RwLock;

use chrono::NaiveDate;
use uuid::Uuid;

/// 抽象快取行為：UUID 失效檢查 + 查詢 + 計算並存入
/// 所有同步細節封裝在各自的實作裡
pub trait CacheBackend {
    fn get_or_compute(
        &self,
        incoming_id: Uuid,
        fixing_date: NaiveDate,
        compute: impl FnOnce() -> f64,
    ) -> f64;
}

// ── 單執行緒版：RefCell ──────────────────────────────────────────────────────

struct CacheInner {
    curve_id: Option<Uuid>,
    cache: HashMap<NaiveDate, f64>,
}

pub struct RefCellBackend {
    inner: RefCell<CacheInner>,
}

impl RefCellBackend {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(CacheInner {
                curve_id: None,
                cache: HashMap::new(),
            }),
        }
    }
}

impl CacheBackend for RefCellBackend {
    fn get_or_compute(
        &self,
        incoming_id: Uuid,
        fixing_date: NaiveDate,
        compute: impl FnOnce() -> f64,
    ) -> f64 {
        let mut inner = self.inner.borrow_mut();

        if inner.curve_id != Some(incoming_id) {
            inner.cache.clear();
            inner.curve_id = Some(incoming_id);
        }

        // entry API 讓查詢跟寫入合併成一步，不需要查兩次
        *inner.cache.entry(fixing_date).or_insert_with(compute)
    }
}

// ── 多執行緒版：RwLock ───────────────────────────────────────────────────────

pub struct RwLockBackend {
    curve_id: RwLock<Option<Uuid>>,
    cache: RwLock<HashMap<NaiveDate, f64>>,
}

impl RwLockBackend {
    pub fn new() -> Self {
        Self {
            curve_id: RwLock::new(None),
            cache: RwLock::new(HashMap::new()),
        }
    }
}

impl CacheBackend for RwLockBackend {
    fn get_or_compute(
        &self,
        incoming_id: Uuid,
        fixing_date: NaiveDate,
        compute: impl FnOnce() -> f64,
    ) -> f64 {
        // Step 1：讀鎖快速確認是否需要 invalidate
        let needs_invalidation = self.curve_id
            .read().unwrap()
            .map_or(true, |id| id != incoming_id);

        // Step 2：Double-checked locking，避免重複清快取
        if needs_invalidation {
            let mut id_w = self.curve_id.write().unwrap();
            if *id_w != Some(incoming_id) {
                self.cache.write().unwrap().clear();
                *id_w = Some(incoming_id);
            }
        }

        // Step 3：讀鎖查快取
        if let Some(&rate) = self.cache.read().unwrap().get(&fixing_date) {
            return rate;
        }

        // Step 4：持鎖外計算，允許其他執行緒並發讀取
        // 同 UUID 下 curve 不可變，兩條執行緒算同一個 fixing_date 結果相同，覆蓋無害
        let rate = compute();
        self.cache.write().unwrap().insert(fixing_date, rate);
        rate
    }
}