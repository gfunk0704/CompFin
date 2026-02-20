// ── cache_backend.rs ────────────────────────────────────────────────────────

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::RwLock;

use chrono::NaiveDate;

/// 抽象快取行為：pointer-identity 失效檢查 + 查詢 + 計算並存入。
///
/// # Cache key 設計：Arc pointer address（usize）
///
/// 原本使用 `Uuid` 作為 curve 身份識別，需要每個 curve 都實作 `ObjectWithUUID`。
/// 但 UUID 的本質只是「pointer identity」——想知道 forward curve 是否換了。
/// Rust 提供更直接的方式：
///
/// ```rust
/// let id = Arc::as_ptr(&forward_curve) as usize;
/// ```
///
/// `Arc::as_ptr()` 回傳指向底層資料的原始指標，只要是同一個 `Arc`（或其 clone），
/// 位址就相同；換了新的 curve 物件位址就不同，正好符合快取失效的語意。
///
/// 好處：
/// - 移除 `ObjectWithUUID` trait 的依賴
/// - 不需要在每個 curve struct 存 `Uuid` field
/// - 呼叫端：`Arc::as_ptr(&curve) as usize` 一行即可，語意明確
pub trait CacheBackend {
    fn get_or_compute(
        &self,
        curve_ptr: usize,   // Arc::as_ptr(&forward_curve) as usize
        fixing_date: NaiveDate,
        compute: impl FnOnce() -> f64,
    ) -> f64;
}

// ── 單執行緒版：RefCell ──────────────────────────────────────────────────────

struct CacheInner {
    curve_ptr: Option<usize>,
    cache: HashMap<NaiveDate, f64>,
}

pub struct RefCellBackend {
    inner: RefCell<CacheInner>,
}

impl RefCellBackend {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(CacheInner {
                curve_ptr: None,
                cache: HashMap::new(),
            }),
        }
    }
}

impl CacheBackend for RefCellBackend {
    fn get_or_compute(
        &self,
        curve_ptr: usize,
        fixing_date: NaiveDate,
        compute: impl FnOnce() -> f64,
    ) -> f64 {
        let mut inner = self.inner.borrow_mut();

        if inner.curve_ptr != Some(curve_ptr) {
            inner.cache.clear();
            inner.curve_ptr = Some(curve_ptr);
        }

        *inner.cache.entry(fixing_date).or_insert_with(compute)
    }
}

// ── 多執行緒版：RwLock ───────────────────────────────────────────────────────
//
// # 已知 trade-off：double-compute
//
// Step 3（read lock）和 Step 4（write lock）之間，兩條執行緒可能都發現 key 不存在，
// 分別計算後先後寫入。由於同一 curve（相同 pointer）+ 同一 fixing_date 結果具確定性，
// 雙重計算只是浪費而非錯誤。
//
// 若需要嚴格的 compute-once 語意，可改用 DashMap + OnceLock，但對此場景過度設計。

pub struct RwLockBackend {
    curve_ptr: RwLock<Option<usize>>,
    cache: RwLock<HashMap<NaiveDate, f64>>,
}

impl RwLockBackend {
    pub fn new() -> Self {
        Self {
            curve_ptr: RwLock::new(None),
            cache: RwLock::new(HashMap::new()),
        }
    }
}

impl CacheBackend for RwLockBackend {
    fn get_or_compute(
        &self,
        curve_ptr: usize,
        fixing_date: NaiveDate,
        compute: impl FnOnce() -> f64,
    ) -> f64 {
        // Step 1：讀鎖快速確認是否需要 invalidate
        let needs_invalidation = self.curve_ptr
            .read().unwrap()
            .map_or(true, |ptr| ptr != curve_ptr);

        // Step 2：Double-checked locking
        if needs_invalidation {
            let mut ptr_w = self.curve_ptr.write().unwrap();
            if *ptr_w != Some(curve_ptr) {
                self.cache.write().unwrap().clear();
                *ptr_w = Some(curve_ptr);
            }
        }

        // Step 3：讀鎖查快取
        if let Some(&rate) = self.cache.read().unwrap().get(&fixing_date) {
            return rate;
        }

        // Step 4：持鎖外計算（允許其他執行緒並發讀取），再寫入
        let rate = compute();
        self.cache.write().unwrap().insert(fixing_date, rate);
        rate
    }
}
