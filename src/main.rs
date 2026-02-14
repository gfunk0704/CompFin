
use chrono::NaiveDate;

use compfin::configuration::Configuration;
use compfin::manager::manager::IManager;
use compfin::time::period::Period;

const JSON_FOLDER:  &'static str = "C:/Users/luray/Dropbox/github/CompFin/compfin/json/";

fn main() {
    
    let mut config_path = JSON_FOLDER.to_owned();
    config_path.push_str("config.json");
    let config = Configuration::new();
    let _ = config.from_reader(config_path);
    let generator_name = "3MModifiedFollowingShift0BD".to_owned();
    let schdeule_generator = config.
        schedule_generator_manager().
        get(&generator_name).
        unwrap();
    let horizon = NaiveDate::from_ymd_opt(2025, 11, 3).unwrap();
    let maturity = Period::parse("10Y".to_owned()).unwrap();
    let calendar_name = "NewYorkBank".to_owned();
    let calendar_nyb = config.
        holiyday_calendar_manager().
        get(&calendar_name).
        unwrap();
    let schdeule = schdeule_generator.
        generate_from_maturity_tenor(
            horizon, 
            maturity, 
            &calendar_nyb, 
            &calendar_nyb, 
            &calendar_nyb).
        unwrap();
    let day_counter_name = "Actual365Fixed".to_owned();
    let day_counter_generator = config.
        day_counter_generator_manager().
        get(&day_counter_name).
        unwrap();
    let day_counter = day_counter_generator.
        generate(None).
        unwrap();
    for schedule_period in schdeule.schedule_periods() {
        let d1 = schedule_period.calculation_period().start_date();
        let d2 = schedule_period.calculation_period().end_date();
        println!("{}, {}, {}, {}, {}",
                 schedule_period.fixing_date(), 
                 schedule_period.calculation_period().start_date(), 
                 schedule_period.calculation_period().end_date(), 
                 schedule_period.payment_date(),
                 day_counter.year_fraction(d1, d2));
    }
} 
   