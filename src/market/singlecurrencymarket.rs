use std::sync::Arc;

use serde::Deserialize;

use crate::manager::manager::{FrozenManager, JsonLoader, ManagerBuilder};
use crate::manager::managererror::{ManagerError, parse_json_value};
use crate::manager::namedobject::Named;
use crate::market::currency::Currency;
use crate::market::market::Market;
use crate::time::calendar::holidaycalendar::HolidayCalendar;

pub struct SingleCurrcneyMarket {
    discount_curve_name: String,
    settlement_calendar: Arc<dyn HolidayCalendar>,
    settlement_currency: Currency,
    settlement_days: u32
}

impl SingleCurrcneyMarket {
    pub fn new(discount_curve_name: String,
               settlement_calendar: Arc<dyn HolidayCalendar>,
               settlement_currency: Currency,
               settlement_days: u32) -> SingleCurrcneyMarket {
        SingleCurrcneyMarket {
            discount_curve_name: discount_curve_name,
            settlement_calendar: settlement_calendar,
            settlement_currency: settlement_currency,
            settlement_days: settlement_days
        }
    }
}

impl Market for SingleCurrcneyMarket {
    fn discount_curve_name(&self) -> &String {
        &self.discount_curve_name
    }

    fn settlement_calendar(&self) -> Arc<(dyn HolidayCalendar + 'static)> {
        Arc::clone(&self.settlement_calendar)
    }

    fn settlement_currency(&self) -> &Currency {
        &self.settlement_currency
    }

    fn settlement_days(&self) -> u32 {
        self.settlement_days
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// SingleCurrencyMarketLoader
// ─────────────────────────────────────────────────────────────────────────────
//
// Supports：&FrozenManager<dyn HolidayCalendar + Send + Sync>
//
// JSON 範例：
//   {
//     "name":                 "TWD_MARKET",
//     "discount_curve_name":  "TWD_OIS",
//     "settlement_calendar":  "TWD",
//     "settlement_currency":  { "code": "TWD", "digits": 0 },
//     "settlement_days":      2
//   }

#[derive(Deserialize)]
struct SingleCurrencyMarketJsonProp {
    discount_curve_name:  String,
    /// 對應到 FrozenManager<dyn HolidayCalendar> 的鍵名。
    settlement_calendar:  String,
    settlement_currency:  Currency,
    settlement_days:      u32,
}

pub struct SingleCurrencyMarketLoader;

impl<'a> JsonLoader<
    dyn Market,
    &'a FrozenManager<dyn HolidayCalendar + Send + Sync>,
> for SingleCurrencyMarketLoader {
    fn insert_obj_from_json(
        &self,
        builder: &mut ManagerBuilder<dyn Market>,
        json_value: serde_json::Value,
        supports: &&'a FrozenManager<dyn HolidayCalendar + Send + Sync>,
    ) -> Result<(), ManagerError> {
        let named: Named<SingleCurrencyMarketJsonProp> = parse_json_value(json_value)?;
        let p = named.inner;

        let calendar = supports.get(&p.settlement_calendar)?;
        let market = SingleCurrcneyMarket::new(
            p.discount_curve_name,
            calendar,
            p.settlement_currency,
            p.settlement_days,
        );
        builder.insert(named.name, Arc::new(market));
        Ok(())
    }
}