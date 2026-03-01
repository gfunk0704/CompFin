use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use serde_json;

use super::managererror::{ManagerError, parse_json_value};
use super::namedobject::NamedJsonObject;

// ═════════════════════════════════════════════════════════════════════════════
// 設計說明
// ═════════════════════════════════════════════════════════════════════════════
//
// 原始設計的問題：
//   Manager<V> 把「載入邏輯」與「資料儲存」耦合在同一個 struct 裡，
//   且 IManager::map() 把內部鎖暴露到 trait 介面，導致：
//   - 呼叫方可任意取得 RefMut 進行寫入（沒有不可變保證）
//   - 無法在 multi-thread 環境使用（RefCell 不是 Send）
//   - get() 強制要求 V: Clone
//
// 新設計：兩階段分離
//
//   ┌── 載入階段（單執行緒，有序） ──────────────────────────────────────────┐
//   │                                                                      │
//   │   ManagerBuilder<V>     ← IManager 實作對象，可寫入                  │
//   │         │                                                            │
//   │         │  build()                                                   │
//   │         ↓                                                            │
//   └── 執行階段（多執行緒，唯讀） ──────────────────────────────────────────┘
//       FrozenManager<V>      ← 零鎖，Arc<HashMap> 不可變，可自由 Clone
//
// 型別對照表：
//   舊：Manager<ScheduleGenerator>          → SimpleLoader → FrozenManager<ScheduleGenerator>
//   舊：Manager<Rc<DayCounterGenerator>>    → SimpleLoader → FrozenManager<DayCounterGenerator>
//   舊：HolidayCalendarManager              → HolidayCalendarLoader (覆寫 insert_obj_from_json_vec)
//                                          → FrozenManager<dyn HolidayCalendar + Send + Sync>
//   舊：InterestRateIndexManager            → InterestRateIndexLoader (自定義 supports)
//                                          → FrozenManager<dyn InterestRateIndex + Send + Sync>


// ─────────────────────────────────────────────────────────────────────────────
// ManagerBuilder
// ─────────────────────────────────────────────────────────────────────────────

/// 載入階段的可寫容器。
///
/// 所有資料載入（JSON 解析、相依解析）都在此結構上進行。
/// 載入完成後呼叫 [`build()`](ManagerBuilder::build) 一次性轉換為
/// [`FrozenManager`]，此後任何 `ManagerBuilder` 的方法皆無法存取凍結後的資料。
///
/// # 泛型參數
/// `V` 可以是具體型別（`ScheduleGenerator`）或 trait object（`dyn HolidayCalendar`）。
/// 使用 trait object 時需確保 trait 有 `Send + Sync` supertrait。
///
/// # 多執行緒安全性
/// `ManagerBuilder` 本身**不**是 `Send`，設計上只在單執行緒的初始化階段使用。
pub struct ManagerBuilder<V: ?Sized + Send + Sync> {
    map: HashMap<String, Arc<V>>,
}

impl<V: ?Sized + Send + Sync> ManagerBuilder<V> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// 插入一個具名物件。若名稱已存在則覆蓋。
    pub fn insert(&mut self, name: String, value: Arc<V>) {
        self.map.insert(name, value);
    }

    /// 在載入階段查詢已載入的物件。
    ///
    /// 主要用途：解析有相依關係的物件（如 `JointCalendar` 需要已載入的兩個 `SimpleCalendar`）。
    pub fn get(&self, name: &str) -> Result<Arc<V>, ManagerError> {
        self.map
            .get(name)
            .cloned()
            .ok_or_else(|| ManagerError::NotFound(name.to_owned()))
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// 凍結 builder，消耗 self 並回傳不可變的執行階段容器。
    ///
    /// 呼叫後 builder 不再可用（所有權移入 FrozenManager 的 Arc）。
    pub fn build(self) -> FrozenManager<V> {
        FrozenManager {
            map: Arc::new(self.map),
        }
    }
}

impl<V: ?Sized + Send + Sync> Default for ManagerBuilder<V> {
    fn default() -> Self {
        Self::new()
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// FrozenManager
// ─────────────────────────────────────────────────────────────────────────────

/// 執行階段的唯讀容器。
///
/// # 效能保證
/// - `get()` 完全無鎖：`Arc<HashMap>` 本身不可變，多執行緒可完全並行讀取
/// - `Clone` 為 O(1)：只是 `Arc::clone`，共享底層 HashMap
///
/// # 不可變保證
/// 沒有任何 `&mut self` 或 `write()` 方法。
/// 由 Rust 型別系統在編譯期保證凍結後不可被修改。
pub struct FrozenManager<V: ?Sized + Send + Sync> {
    map: Arc<HashMap<String, Arc<V>>>,
}

// #[derive(Clone)] 會對 V 加上 V: Clone 的約束，不適用於 ?Sized 場景，需手動實作。
impl<V: ?Sized + Send + Sync> Clone for FrozenManager<V> {
    /// O(1) clone：只複製 Arc，不複製底層 HashMap。
    fn clone(&self) -> Self {
        FrozenManager {
            map: Arc::clone(&self.map),
        }
    }
}

impl<V: ?Sized + Send + Sync> FrozenManager<V> {
    /// 查詢具名物件，回傳 Arc 共享所有權（不複製物件本身）。
    pub fn get(&self, name: &str) -> Result<Arc<V>, ManagerError> {
        self.map
            .get(name)
            .cloned()
            .ok_or_else(|| ManagerError::NotFound(name.to_owned()))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// FrozenManager 是 Send + Sync：Arc<HashMap<String, Arc<V>>> 在 V: Send + Sync 時為 Send + Sync
unsafe impl<V: ?Sized + Send + Sync> Send for FrozenManager<V> {}
unsafe impl<V: ?Sized + Send + Sync> Sync for FrozenManager<V> {}


// ─────────────────────────────────────────────────────────────────────────────
// IManager
// ─────────────────────────────────────────────────────────────────────────────

/// Manager 的載入介面。
///
/// 負責「如何從 JSON 建立物件」，與「物件的儲存方式」完全解耦。
/// 實作方只需關注物件的建立邏輯，不需要知道物件最終如何被儲存或存取。
///
/// # 型別參數
/// - `V`: 被管理的物件型別（可以是 trait object，如 `dyn HolidayCalendar`）
/// - `S`: 相依的外部資料（其他 FrozenManager 或設定），無相依時使用 `()`
///
/// # 設計意圖
/// 相對於舊的 `IManager`，新版本：
/// - 移除 `map()` —— 儲存細節不應洩漏到介面
/// - 移除 `get()` —— 查詢由 `FrozenManager` 負責
/// - `insert_obj_from_json` 改為操作 `&mut ManagerBuilder<V>`，明確表達只在載入階段可寫
/// - `V: Clone` 約束移除，改用 `Arc<V>` 共享所有權
///
/// # 範例：有相依性的 JointCalendar 重試邏輯
/// ```rust
/// // 覆寫 insert_obj_from_json_vec，實作自定義載入順序
/// fn insert_obj_from_json_vec(
///     &self,
///     builder: &mut ManagerBuilder<dyn HolidayCalendar + Send + Sync>,
///     json_vec: &[serde_json::Value],
///     supports: &(),
/// ) -> Result<(), ManagerError> {
///     let mut remain = (0..json_vec.len()).collect::<Vec<_>>();
///     loop {
///         let mut failed = Vec::new();
///         for &i in &remain {
///             if self.insert_obj_from_json(builder, json_vec[i].clone(), supports).is_err() {
///                 failed.push(i);
///             }
///         }
///         if failed.is_empty() || failed == remain { break; }
///         remain = failed;
///     }
///     Ok(())
/// }
/// ```
pub trait IManager<V: ?Sized + Send + Sync, S> {

    /// 從單一 JSON 物件解析並插入 builder。
    ///
    /// 實作者需：
    /// 1. 從 `json_value` 讀取 `name` 欄位
    /// 2. 利用 `supports` 解析相依物件（如查詢其他 manager）
    /// 3. 呼叫 `builder.insert(name, Arc::new(obj))`
    fn insert_obj_from_json(
        &self,
        builder: &mut ManagerBuilder<V>,
        json_value: serde_json::Value,
        supports: &S,
    ) -> Result<(), ManagerError>;

    /// 批次載入。預設為循序插入，無相依性時已足夠。
    ///
    /// 若物件之間有載入順序相依（如 `JointCalendar` 需要兩個 `SimpleCalendar` 先載入），
    /// 可覆寫此方法實作重試邏輯（參見 trait 文件的範例）。
    fn insert_obj_from_json_vec(
        &self,
        builder: &mut ManagerBuilder<V>,
        json_vec: &[serde_json::Value],
        supports: &S,
    ) -> Result<(), ManagerError> {
        for j in json_vec {
            self.insert_obj_from_json(builder, j.clone(), supports)?;
        }
        Ok(())
    }

    /// 從 JSON 檔案載入（支援 array 或單一 object）。
    ///
    /// 原本的 `from_reader` 參數為 `String`，改為 `&str` 更通用。
    fn load_from_reader(
        &self,
        builder: &mut ManagerBuilder<V>,
        file_path: &str,
        supports: &S,
    ) -> Result<(), ManagerError> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let json_value: serde_json::Value = serde_json::from_reader(reader)
            ?;

        if let Some(arr) = json_value.as_array() {
            self.insert_obj_from_json_vec(builder, arr, supports)
        } else {
            self.insert_obj_from_json(builder, json_value, supports)
        }
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// SimpleLoader
// ─────────────────────────────────────────────────────────────────────────────

/// 基於工廠函式的簡單載入器，取代原本的 `Manager<V>`。
///
/// 適用於**無外部相依性**、可直接從單一 JSON 物件建立的型別
/// （如 `ScheduleGenerator`、`DayCounterGenerator`）。
///
/// # 遷移指南
///
/// 舊寫法：
/// ```rust
/// // DayCounterGeneratorManager
/// pub fn new() -> Manager<Rc<DayCounterGenerator>> {
///     Manager::new(get_day_counter_generator_from_json)
/// }
/// // 工廠型別：fn(Value) -> Result<Rc<DayCounterGenerator>, ManagerError>
/// ```
///
/// 新寫法：
/// ```rust
/// // 工廠型別改為回傳 Arc（移除 Rc）：
/// fn get_day_counter_generator_from_json(v: Value) -> Result<Arc<DayCounterGenerator>, ManagerError> { ... }
///
/// pub fn new_loader() -> SimpleLoader<DayCounterGenerator> {
///     SimpleLoader::new(get_day_counter_generator_from_json)
/// }
/// // 使用：
/// let mut builder = ManagerBuilder::new();
/// new_loader().insert_obj_from_json_vec(&mut builder, &json_vec, &())?;
/// let frozen: FrozenManager<DayCounterGenerator> = builder.build();
/// ```
///
/// # 注意
/// 工廠函式的回傳型別從 `Result<V, ManagerError>` 改為 `Result<Arc<V>, ManagerError>`。
/// 內部若需要 `Rc`（如 `DayCounterNumeratorGenerator`），請在工廠函式內部保留 `Rc`，
/// 外層改用 `Arc` 包裝最終產物即可。
pub struct SimpleLoader<V: Send + Sync + 'static> {
    factory: fn(serde_json::Value) -> Result<Arc<V>, ManagerError>,
}

impl<V: Send + Sync + 'static> SimpleLoader<V> {
    pub fn new(factory: fn(serde_json::Value) -> Result<Arc<V>, ManagerError>) -> Self {
        Self { factory }
    }
}

impl<V: Send + Sync + 'static> IManager<V, ()> for SimpleLoader<V> {
    fn insert_obj_from_json(
        &self,
        builder: &mut ManagerBuilder<V>,
        json_value: serde_json::Value,
        _supports: &(),
    ) -> Result<(), ManagerError> {
        let named: NamedJsonObject =
            parse_json_value(json_value.clone())?;
        let v = (self.factory)(json_value)?;
        builder.insert(named.name().to_owned(), v);
        Ok(())
    }
}
