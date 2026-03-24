use std::fs::File;
use std::io::BufReader;

use serde::Deserialize;

use crate::instrument::interestrate::deposit::DepositGenerator;
use crate::instrument::interestrate::deposit::DepositGeneratorLoader;
use crate::instrument::interestrate::interestrateswap::InterestRateSwapGenerator;
use crate::instrument::interestrate::interestrateswap::InterestRateSwapGeneratorLoader;
use crate::instrument::leg::legcharactersgeneratorloader::InterestRateInstrumentSupports;
use crate::interestrate::index::interestrateindex::InterestRateIndex;
use crate::interestrate::index::interestrateindexmanager::InterestRateIndexLoader;
use crate::manager::manager::{
    FrozenManager,
    JsonLoader,
    ManagerBuilder,
};
use crate::manager::managererror::ManagerError;
use crate::market::market::Market;
use crate::market::singlecurrencymarket::SingleCurrencyMarketLoader;
use crate::time::calendar::holidaycalendar::HolidayCalendar;
use crate::time::calendar::holidaycalendarmanager::HolidayCalendarLoader;
use crate::time::daycounter::daycounter::DayCounterGenerator;
use crate::time::daycounter::daycountergeneratormanager::DayCounterGeneratorManager;
use crate::time::schedule::schedule::ScheduleGenerator;
use crate::time::schedule::schedulegeneratormanager::ScheduleGeneratorManager;


pub struct InterestRateInstrumentGeneratorCollection {
    pub deposit_generator_manager: FrozenManager<DepositGenerator>,
    pub swap_generator_manager:    FrozenManager<InterestRateSwapGenerator>,
}


pub struct InstrumentGeneratorCollection {
    pub interest_rate: InterestRateInstrumentGeneratorCollection,
}


#[derive(Deserialize)]
struct ConfigurationJsonProp {
    holiday_calendar:      Vec<serde_json::Value>,
    schedule:              Vec<serde_json::Value>,
    day_count:             Vec<serde_json::Value>,
    market:                Vec<serde_json::Value>,
    interest_rate_index:   Vec<serde_json::Value>,
    deposit_generator:     Vec<serde_json::Value>,
    swap_generator:        Vec<serde_json::Value>,
}

/// 系統設定容器。
///
/// 在 `from_reader` 中完成所有載入與凍結，
/// 之後所有 manager 均為不可變的 `FrozenManager`，可安全跨執行緒共享。
///
/// # 載入順序
///
/// 各 manager 之間有相依關係，必須按照以下順序建立：
///
/// 1. `holiday_calendar`
/// 2. `schedule` / `day_count`（互不相依，順序可調換）
/// 3. `interest_rate_index`（依賴 calendar、day_count）
/// 4. `market`（依賴 calendar）
/// 5. `deposit_generator` / `swap_generator`（依賴 market、calendar、schedule、day_count、index）
pub struct Configuration {
    holiday_calendar_manager:      FrozenManager<dyn HolidayCalendar + Send + Sync>,
    schedule_generator_manager:    FrozenManager<ScheduleGenerator>,
    day_counter_generator_manager: FrozenManager<DayCounterGenerator>,
    market_manager:                FrozenManager<dyn Market>,
    interest_rate_index_manager:   FrozenManager<dyn InterestRateIndex + Send + Sync>,
    instrument_generator_collection: InstrumentGeneratorCollection,
}

impl Configuration {
    pub fn from_reader(file_path: &str) -> Result<Configuration, ManagerError> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let json_prop: ConfigurationJsonProp = serde_json::from_reader(reader)?;

        // ── 1. Holiday Calendar ───────────────────────────────────────────────
        let mut cal_builder: ManagerBuilder<dyn HolidayCalendar + Send + Sync> =
            ManagerBuilder::new();
        HolidayCalendarLoader
            .insert_obj_from_json_vec(&mut cal_builder, &json_prop.holiday_calendar, &())?;
        let holiday_calendar_manager = cal_builder.build();

        // ── 2. Schedule Generator ─────────────────────────────────────────────
        let mut sched_builder: ManagerBuilder<ScheduleGenerator> = ManagerBuilder::new();
        ScheduleGeneratorManager::new_loader()
            .insert_obj_from_json_vec(&mut sched_builder, &json_prop.schedule, &())?;
        let schedule_generator_manager = sched_builder.build();

        // ── 3. Day Counter Generator ──────────────────────────────────────────
        let mut dc_builder: ManagerBuilder<DayCounterGenerator> = ManagerBuilder::new();
        DayCounterGeneratorManager::new_loader()
            .insert_obj_from_json_vec(&mut dc_builder, &json_prop.day_count, &())?;
        let day_counter_generator_manager = dc_builder.build();

        // ── 4. Market ─────────────────────────────────────────────────────────
        let mut mkt_builder: ManagerBuilder<dyn Market> = ManagerBuilder::new();
        SingleCurrencyMarketLoader
            .insert_obj_from_json_vec(
                &mut mkt_builder,
                &json_prop.market,
                &&holiday_calendar_manager,
            )?;
        let market_manager = mkt_builder.build();

        // ── 5. Interest Rate Index ────────────────────────────────────────────
        let mut idx_builder: ManagerBuilder<dyn InterestRateIndex + Send + Sync> =
            ManagerBuilder::new();
        let index_supports = (&holiday_calendar_manager, &day_counter_generator_manager);
        InterestRateIndexLoader
            .insert_obj_from_json_vec(
                &mut idx_builder,
                &json_prop.interest_rate_index,
                &index_supports,
            )?;
        let interest_rate_index_manager = idx_builder.build();

        // ── 6. Instrument Generators ──────────────────────────────────────────
        let ir_supports: InterestRateInstrumentSupports = (
            &market_manager,
            &holiday_calendar_manager,
            &schedule_generator_manager,
            &day_counter_generator_manager,
            &interest_rate_index_manager,
        );

        let mut dep_builder: ManagerBuilder<DepositGenerator> = ManagerBuilder::new();
        DepositGeneratorLoader
            .insert_obj_from_json_vec(
                &mut dep_builder,
                &json_prop.deposit_generator,
                &ir_supports,
            )?;
        let deposit_generator_manager = dep_builder.build();

        let mut swap_builder: ManagerBuilder<InterestRateSwapGenerator> = ManagerBuilder::new();
        InterestRateSwapGeneratorLoader
            .insert_obj_from_json_vec(
                &mut swap_builder,
                &json_prop.swap_generator,
                &ir_supports,
            )?;
        let swap_generator_manager = swap_builder.build();

        let instrument_generator_collection = InstrumentGeneratorCollection {
            interest_rate: InterestRateInstrumentGeneratorCollection {
                deposit_generator_manager,
                swap_generator_manager,
            },
        };

        Ok(Configuration {
            holiday_calendar_manager,
            schedule_generator_manager,
            day_counter_generator_manager,
            market_manager,
            interest_rate_index_manager,
            instrument_generator_collection,
        })
    }

    pub fn holiday_calendar_manager(&self) -> &FrozenManager<dyn HolidayCalendar + Send + Sync> {
        &self.holiday_calendar_manager
    }

    pub fn schedule_generator_manager(&self) -> &FrozenManager<ScheduleGenerator> {
        &self.schedule_generator_manager
    }

    pub fn day_counter_generator_manager(&self) -> &FrozenManager<DayCounterGenerator> {
        &self.day_counter_generator_manager
    }

    pub fn market_manager(&self) -> &FrozenManager<dyn Market> {
        &self.market_manager
    }

    pub fn interest_rate_index_manager(
        &self,
    ) -> &FrozenManager<dyn InterestRateIndex + Send + Sync> {
        &self.interest_rate_index_manager
    }

    pub fn instrument_generator_collection(&self) -> &InstrumentGeneratorCollection {
        &self.instrument_generator_collection
    }

    /// 取出利率商品的 [`InterestRateInstrumentSupports`]，供 quote loader 使用。
    pub fn interest_rate_instrument_supports(&self) -> InterestRateInstrumentSupports {
        (
            &self.market_manager,
            &self.holiday_calendar_manager,
            &self.schedule_generator_manager,
            &self.day_counter_generator_manager,
            &self.interest_rate_index_manager,
        )
    }
}
