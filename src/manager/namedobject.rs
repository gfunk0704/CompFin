use serde::Deserialize;

/// JSON 物件的泛型具名包裝。
///
/// 以 `#[serde(flatten)]` 把 `inner` 的欄位展開到同一層 JSON 物件，
/// 因此 JSON 格式與直接在各 prop struct 裡加 `name` 欄位完全相同，
/// 但只需要 deserialize 一次，且由型別系統強制保證 `name` 的存在。
///
/// # 範例
/// ```json
/// { "name": "USD_MARKET", "discount_curve": "USD_OIS", ... }
/// ```
/// ```rust
/// let named: Named<MarketJsonProp> = parse_json_value(json_value)?;
/// builder.insert(named.name, Arc::new(...));
/// ```
#[derive(Deserialize)]
pub struct Named<T> {
    pub name: String,
    #[serde(flatten)]
    pub inner: T,
}
