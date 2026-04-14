// ── iterativebootstrapper.rs ──────────────────────────────────────────────────
//
// 逐點拔靴法（Iterative Bootstrapping）曲線校準器。
//
// # 設計說明
//
// 從最短天期的校準商品開始，依序求解每個 pillar 的利率值，
// 使對應商品的 NPV = 0。每解出一個 pillar 後，將該值固定，
// 再用已建構完成的部分曲線繼續求解下一個 pillar。
//
// # 第一個 pillar 的特殊處理
//
// PiecewisePolynomial 至少需要兩個點才能建構，因此第一個 pillar
// 使用 FlatForwardCurve（只需一個參數：常數利率）。求解完成後，
// 將結果依照 InterpolationTarget 轉換後存入 solved_values，
// 確保第二個 pillar 開始切換回 PiecewisePoly 時數值一致。
//
// 左外插強制為 FlatForwardRate，保證切換時曲線的連續性。
//
// # 初值與 Bracket 策略
//
// 初值根據 InterpolationTarget 從 market_rate 推導：
//   ZeroRate / InstForward → market_rate
//   LogDiscount → -market_rate × τ
//
// Bracket 使用 margin = max(|initial| × 0.5, 1e-4)，
// 確保零利率、負利率環境下不退化。
//
// # Partial Freeze 優化（`apply_partial_freeze_cash_flows`）
//
// 當啟用時，從第二個 pillar 開始：
//   1. 用前一個 pillar 日期作為 cutoff，以已固定曲線計算
//      cutoff 之前的 projected flows NPV（frozen prefix NPV）
//   2. root solver 迭代只計算 cutoff 之後的 tail flows NPV
//   3. 總 NPV = frozen_prefix_npv + tail_npv
//
// 對長天期 IRS（如 10Y quarterly float = 40 期），
// 解最後一個 pillar 時只需重算最後 1 期，
// 省略前 39 期的 CompoundingRateIndex 逐日計算。

use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;

use crate::configuration::InterestRateInstrumentGeneratorCollection;
use crate::instrument::instrument::{
    CurveFunction,
    Position,
    SimpleInstrument,
};
use crate::marketdata::interestrate::interestratequotesheet::InterestRateQuoteSheet;
use crate::math::rootsolver::{RootSolver, RootSolverConfig};
use crate::model::interestrate::bootstrappingtrait::BootstrappingTrait;
use crate::model::interestrate::flatforwardcurve::FlatForwardCurve;
use crate::model::interestrate::interestratecurve::{
    InterestRateCurve,
    InterestRateCurveGenerator,
    YearFractionCalculator,
};
use crate::model::interestrate::interestratecurvecalibrator::{
    CalibrationError,
    InterestRateCurveCalibrationHelper,
    InterestRateCurvePillar,
    InterestRateCurveCalibrator,
};
use crate::model::interestrate::piecewisepolyinterestratecurve::{
    ExtrapolationMethod,
    PiecewisePolyInterestRateCurveGenerator,
};
use crate::pricer::pricer::Pricer;
use crate::pricer::simpleinstrumentpricer::SimpleInstrumentPricer;
use crate::pricingcondition::{DecimalRounding, PricingCondition};
use crate::time::daycounter::daycounter::DayCounterGenerator;


// ─────────────────────────────────────────────────────────────────────────────
// IterativeBootstrapper
// ─────────────────────────────────────────────────────────────────────────────

pub struct IterativeBootstrapper {
    root_solver_config:              RootSolverConfig,
    bootstrapping_trait:             BootstrappingTrait,
    day_counter_generator:           Arc<DayCounterGenerator>,
    apply_partial_freeze_cash_flows: bool,
}

impl IterativeBootstrapper {
    /// 從 PiecewisePolyInterestRateCurveGenerator 建構 IterativeBootstrapper。
    ///
    /// # 參數
    /// - `apply_partial_freeze_cash_flows`: 啟用 partial freeze 優化。
    ///   當 `true` 時，第二個 pillar 開始使用凍結前段 NPV 加速求解。
    ///
    /// # Errors
    ///
    /// 若 generator 的 left_extrapolation 不是 FlatForwardRate，
    /// 回傳 CalibrationError。IterativeBootstrapping 要求左外插為
    /// FlatForwardRate 以保證第一個 pillar（FlatForwardCurve）與
    /// 後續 pillar（PiecewisePoly）之間的曲線連續性。
    pub fn new(
        root_solver_config:              RootSolverConfig,
        generator:                       &PiecewisePolyInterestRateCurveGenerator,
        apply_partial_freeze_cash_flows: bool,
    ) -> Result<Self, CalibrationError> {
        if generator.left_extrapolation() != ExtrapolationMethod::FlatForwardRate {
            return Err(CalibrationError::CurveGeneration(
                "IterativeBootstrapper requires left_extrapolation = FlatForwardRate \
                 to ensure continuity between FlatForwardCurve (1st pillar) and \
                 PiecewisePoly (subsequent pillars)".to_string()
            ));
        }

        Ok(Self {
            root_solver_config,
            bootstrapping_trait: BootstrappingTrait::new(generator.interpolation_target()),
            day_counter_generator: generator.day_counter_generator().clone(),
            apply_partial_freeze_cash_flows,
        })
    }

    /// 預設不啟用 partial freeze 的建構方式。
    pub fn with_defaults(
        generator: &PiecewisePolyInterestRateCurveGenerator,
    ) -> Result<Self, CalibrationError> {
        Self::new(RootSolverConfig::default(), generator, false)
    }

    /// 為校準建構 PricingCondition。
    ///
    /// horizon = reference_date，保證 DF(horizon) = 1 的不變式。
    /// 校準期間不做 rounding（避免影響數值精度）。
    fn calibration_pricing_condition(horizon: NaiveDate) -> PricingCondition {
        PricingCondition::new(
            horizon,
            true,   // include_horizon_flow
            true,   // estimate_horizon_index
            DecimalRounding::new(false, false, false),
        )
    }

    /// 從校準商品建構 market_data HashMap。
    ///
    /// 單曲線校準假設：商品 curve_name_map 中所有 curve name
    /// 均指向同一條正在被校準的曲線。
    fn build_market_data(
        instrument: &dyn SimpleInstrument,
        curve: &Arc<dyn InterestRateCurve>,
    ) -> HashMap<String, Arc<dyn InterestRateCurve>> {
        let mut market_data = HashMap::new();
        for curve_name in instrument.curve_name_map().values() {
            market_data.insert(curve_name.clone(), curve.clone());
        }
        market_data
    }

    /// 建立 FlatForwardCurve 用的 YearFractionCalculator。
    fn make_yfc(&self, reference_date: NaiveDate) -> Result<YearFractionCalculator, CalibrationError> {
        let day_counter = self.day_counter_generator
            .generate(None)
            .map_err(|e| CalibrationError::CurveGeneration(
                format!("day counter generation failed: {}", e)
            ))?;
        Ok(YearFractionCalculator::new(reference_date, Arc::new(day_counter)))
    }

    /// 第一個 pillar：使用 FlatForwardCurve 求解。
    ///
    /// 回傳已經過 InterpolationTarget 轉換的值，
    /// 可直接加入 solved_values 供後續 PiecewisePoly 使用。
    fn solve_first_pillar(
        &self,
        instrument:        &Arc<dyn SimpleInstrument>,
        market_rate:       f64,
        pillar_date:       NaiveDate,
        pricer:            &SimpleInstrumentPricer,
        solver:            &RootSolver,
        pricing_condition: &PricingCondition,
        yfc:               &YearFractionCalculator,
    ) -> Result<f64, CalibrationError> {
        // FlatForwardCurve 的求解目標是 zero rate（= inst forward under flat forward）
        let initial_guess = market_rate;
        let (lower, upper) = self.bootstrapping_trait.bracket(initial_guess);

        let objective = |rate: f64| -> f64 {
            let curve: Arc<dyn InterestRateCurve> = Arc::new(
                FlatForwardCurve::new(yfc.clone(), rate)
            );
            let market_data = Self::build_market_data(instrument.as_ref(), &curve);
            pricer
                .market_value(instrument.as_ref(), &market_data, pricing_condition)
                .map(|npv| npv.amount())
                .unwrap_or(f64::NAN)
        };

        let solved_rate = solver
            .solve(objective, initial_guess, Some(upper))
            .or_else(|_| solver.solve(objective, lower, Some(upper)))
            .map_err(|e| CalibrationError::CurveGeneration(
                format!("pillar 0 ({:?}) failed: {}", pillar_date, e)
            ))?;

        // 將 FlatForward 的 zero rate 轉換為 PiecewisePoly 的 InterpolationTarget 值
        Ok(self.bootstrapping_trait.convert_flat_forward_to_target(
            solved_rate, yfc, pillar_date,
        ))
    }

    /// 後續 pillar（i ≥ 1）：使用 PiecewisePoly curve generator 求解。
    fn solve_subsequent_pillar(
        &self,
        i:                 usize,
        instrument:        &Arc<dyn SimpleInstrument>,
        market_rate:       f64,
        pillar_date:       NaiveDate,
        pillar_dates:      &[NaiveDate],
        solved_values:     &[f64],
        curve_generator:   &Arc<dyn InterestRateCurveGenerator>,
        reference_date:    NaiveDate,
        pricer:            &SimpleInstrumentPricer,
        solver:            &RootSolver,
        pricing_condition: &PricingCondition,
        yfc:               &YearFractionCalculator,
    ) -> Result<f64, CalibrationError> {
        let initial_guess = self.bootstrapping_trait.initial_value(
            market_rate, yfc, pillar_date,
        );
        let (lower, upper) = self.bootstrapping_trait.bracket(initial_guess);

        let current_dates = &pillar_dates[..=i];

        let objective = |value: f64| -> f64 {
            let mut trial_values = solved_values.to_vec();
            trial_values.push(value);

            let curve = match curve_generator.generate_with_dates(
                reference_date, current_dates, trial_values,
            ) {
                Ok(c) => c,
                Err(_) => return f64::NAN,
            };

            let market_data = Self::build_market_data(instrument.as_ref(), &curve);
            pricer
                .market_value(instrument.as_ref(), &market_data, pricing_condition)
                .map(|npv| npv.amount())
                .unwrap_or(f64::NAN)
        };

        solver
            .solve(objective, initial_guess, Some(upper))
            .or_else(|_| solver.solve(objective, lower, Some(upper)))
            .map_err(|e| CalibrationError::CurveGeneration(
                format!("pillar {} ({:?}) failed: {}", i, pillar_date, e)
            ))
    }

    /// 後續 pillar（i ≥ 1）的 freeze 版本。
    ///
    /// 使用 partial freeze 策略加速：
    ///   1. 用前一個 pillar 的日期作為 cutoff，以已固定的曲線
    ///      計算 cutoff 之前的 projected flows NPV（frozen prefix NPV）
    ///   2. root solver 每次迭代只計算 cutoff 之後的 tail flows NPV
    ///   3. 總 NPV = frozen_prefix_npv + tail_npv
    ///
    /// 對 10Y quarterly float IRS（40 期），解最後一個 pillar 時
    /// 只需重算最後 1 期，省略前 39 期的 CompoundingRateIndex 逐日計算，
    /// 效益約 (n-1)/n。
    fn solve_subsequent_pillar_with_freeze(
        &self,
        i:                 usize,
        instrument:        &Arc<dyn SimpleInstrument>,
        market_rate:       f64,
        pillar_date:       NaiveDate,
        pillar_dates:      &[NaiveDate],
        solved_values:     &[f64],
        curve_generator:   &Arc<dyn InterestRateCurveGenerator>,
        reference_date:    NaiveDate,
        solver:            &RootSolver,
        pricing_condition: &PricingCondition,
        yfc:               &YearFractionCalculator,
    ) -> Result<f64, CalibrationError> {
        let initial_guess = self.bootstrapping_trait.initial_value(
            market_rate, yfc, pillar_date,
        );
        let (lower, upper) = self.bootstrapping_trait.bracket(initial_guess);

        // 用前一個 pillar 的日期作為 freeze cutoff
        let cutoff_date = pillar_dates[i - 1];
        let current_dates = &pillar_dates[..=i];

        // 建構 freeze 時的固定曲線（使用目前已求解的 solved_values）
        let frozen_curve = curve_generator
            .generate_with_dates(reference_date, &pillar_dates[..i], solved_values.to_vec())
            .map_err(|e| CalibrationError::CurveGeneration(
                format!("pillar {} freeze curve generation failed: {}", i, e)
            ))?;

        // 計算 cutoff 之前（含 cutoff）的 frozen prefix NPV
        let curve_name_map = instrument.curve_name_map();
        let pay_forward_opt = curve_name_map
            .get(&CurveFunction::PayForward)
            .map(|_| &frozen_curve);
        let receive_forward_opt = curve_name_map
            .get(&CurveFunction::ReceiveForward)
            .map(|_| &frozen_curve);

        let frozen_pay_flows = instrument.projected_pay_flows_before_equal(
            cutoff_date, pay_forward_opt, pricing_condition,
        );
        let frozen_receive_flows = instrument.projected_receive_flows_before_equal(
            cutoff_date, receive_forward_opt, pricing_condition,
        );

        let horizon = *pricing_condition.horizon();
        let frozen_prefix_npv =
            (frozen_pay_flows + frozen_receive_flows).npv(&frozen_curve, Some(horizon));

        let settlement_date = instrument
            .profit_and_loss_market()
            .settlement_date(horizon);

        let objective = |value: f64| -> f64 {
            let mut trial_values = solved_values.to_vec();
            trial_values.push(value);

            let trial_curve = match curve_generator.generate_with_dates(
                reference_date, current_dates, trial_values,
            ) {
                Ok(c) => c,
                Err(_) => return f64::NAN,
            };

            // tail flows（cutoff_date 之後的部分）使用 trial curve
            let pay_fwd = curve_name_map
                .get(&CurveFunction::PayForward)
                .map(|_| &trial_curve);
            let recv_fwd = curve_name_map
                .get(&CurveFunction::ReceiveForward)
                .map(|_| &trial_curve);

            let tail_pay = instrument.projected_pay_flows_after(
                cutoff_date, pay_fwd, pricing_condition,
            );
            let tail_receive = instrument.projected_receive_flows_after(
                cutoff_date, recv_fwd, pricing_condition,
            );

            let tail_npv = (tail_pay + tail_receive).npv(&trial_curve, Some(horizon));
            let total_npv_at_horizon = frozen_prefix_npv + tail_npv;

            // 與 SimpleInstrumentPricer::market_value 一致的 settlement 折現
            let df_settlement = trial_curve.to_discount_curve().discount(settlement_date);
            total_npv_at_horizon / df_settlement
        };

        solver
            .solve(objective, initial_guess, Some(upper))
            .or_else(|_| solver.solve(objective, lower, Some(upper)))
            .map_err(|e| CalibrationError::CurveGeneration(
                format!("pillar {} ({:?}) freeze solve failed: {}", i, pillar_date, e)
            ))
    }
}


impl InterestRateCurveCalibrator for IterativeBootstrapper {
    fn calibrate(
        &self,
        curve_generator:      Arc<dyn InterestRateCurveGenerator>,
        reference_date:       NaiveDate,
        pillars:              Vec<InterestRateCurvePillar>,
        quote_book:           &HashMap<String, InterestRateQuoteSheet>,
        generator_collection: &InterestRateInstrumentGeneratorCollection,
        position:             Position,
        horizon:              NaiveDate,
    ) -> Result<Arc<dyn InterestRateCurve>, CalibrationError> {
        // 1. 產生所有校準商品（含 market_rate）
        let helpers = Self::generate_calibration_set(
            &pillars,
            quote_book,
            generator_collection,
            position,
            horizon,
        )?;

        // 2. 按 max_date 排序（短天期 → 長天期）
        let mut sorted_helpers: Vec<InterestRateCurveCalibrationHelper> = helpers;
        sorted_helpers.sort_by_key(|h| h.instrument().max_date());

        // 3. 從排序後的 helpers 中取出 pillar dates、market_rates、instruments
        let pillar_dates: Vec<NaiveDate> = sorted_helpers
            .iter()
            .map(|h| h.instrument().max_date())
            .collect();

        let market_rates: Vec<f64> = sorted_helpers
            .iter()
            .map(|h| h.market_rate())
            .collect();

        let sorted_instruments: Vec<Arc<dyn SimpleInstrument>> = sorted_helpers
            .into_iter()
            .map(|h| h.into_instrument())
            .collect();

        let n = pillar_dates.len();
        if n == 0 {
            return Err(CalibrationError::CurveGeneration(
                "no calibration instruments provided".to_string(),
            ));
        }

        // 4. 逐點求解
        let pricer = SimpleInstrumentPricer;
        let solver = RootSolver::new(self.root_solver_config.clone());
        let pricing_condition = Self::calibration_pricing_condition(horizon);
        let yfc = self.make_yfc(reference_date)?;

        let mut solved_values: Vec<f64> = Vec::with_capacity(n);

        for i in 0..n {
            let value = if i == 0 {
                self.solve_first_pillar(
                    &sorted_instruments[i],
                    market_rates[i],
                    pillar_dates[i],
                    &pricer,
                    &solver,
                    &pricing_condition,
                    &yfc,
                )?
            } else if self.apply_partial_freeze_cash_flows {
                self.solve_subsequent_pillar_with_freeze(
                    i,
                    &sorted_instruments[i],
                    market_rates[i],
                    pillar_dates[i],
                    &pillar_dates,
                    &solved_values,
                    &curve_generator,
                    reference_date,
                    &solver,
                    &pricing_condition,
                    &yfc,
                )?
            } else {
                self.solve_subsequent_pillar(
                    i,
                    &sorted_instruments[i],
                    market_rates[i],
                    pillar_dates[i],
                    &pillar_dates,
                    &solved_values,
                    &curve_generator,
                    reference_date,
                    &pricer,
                    &solver,
                    &pricing_condition,
                    &yfc,
                )?
            };

            solved_values.push(value);
        }

        // 5. 用完整的 solved_values 建構最終曲線
        curve_generator
            .generate_with_dates(reference_date, &pillar_dates, solved_values)
            .map_err(|e| CalibrationError::CurveGeneration(e.to_string()))
    }
}
