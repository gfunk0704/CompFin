use std::collections::{
    HashMap, 
    HashSet
};
use std::ops::{AddAssign, Index, IndexMut, MulAssign, SubAssign};
use std::ops::{
    Add,
    Mul,
    Neg,
    Sub
};
use std::rc::Rc;

use chrono::NaiveDate;

use super::super::model::interestrate::interestratecurve::InterestRateCurve;


pub struct CashFlows {
    flows: HashMap<NaiveDate, f64>
}


impl CashFlows {
    pub fn new() -> CashFlows {
        CashFlows { flows: HashMap::new() }
    }

    pub fn from_hash_map(flows_map: HashMap<NaiveDate, f64>) -> CashFlows {
        CashFlows { flows: flows_map.into() }
    }

    pub fn dates(&self) -> HashSet<NaiveDate> {
        self.flows.keys().copied().collect()
    }

    pub fn values(&self) -> Vec<f64> {
        self.flows.values().copied().collect()
    }

    pub fn npv(&self, 
               discount_curve: &Rc<dyn InterestRateCurve>, 
               settlement_date_opt: Option<NaiveDate>) -> f64 {
        
        // 1. 使用迭代器計算所有現金流的現值加總
        let mut total_npv: f64 = self.flows.iter()
            .map(|(date, amount)| amount * discount_curve.discount(date))
            .sum();
        
        // 2. 處理結算日折現 (Settlement Date Discounting)
        if let Some(settlement_date) = settlement_date_opt {
            let df = discount_curve.discount(&settlement_date);
            // 避免除以 0 的安全檢查 (雖然 DF 通常不為 0)
            assert!(df > 0.0, "Discount factor at settlement date must be positive. Check your curve!");
            total_npv /= df;
        }

        total_npv
    }

    pub fn sum(&self) -> f64 {
        // values() 返回迭代器，sum() 是高度優化的消費函數
        self.flows.values().sum()
    }
}


impl Index<&NaiveDate> for CashFlows {
    type Output = f64;
    fn index(&self, date: &NaiveDate) -> &f64 {
        static ZERO: f64 = 0.0;
        // 找不到回傳 0.0 的引用，這符合金融「沒錢就不計入」的邏輯
        self.flows.get(date).unwrap_or(&ZERO)
    }
}


impl IndexMut<&NaiveDate> for CashFlows {
    fn index_mut(&mut self, date: &NaiveDate) -> &mut Self::Output {
        // 如果日期不存在，我們自動插入 0.0 並回傳它的可變引用
        self.flows.entry(*date).or_insert(0.0)
    }
}

macro_rules! impl_cashflows_arithmetic {
    ($trait_op:ident, $method_op:ident, $trait_assign:ident, $method_assign:ident, $op:tt) => {
        // 1. 實作 AddAssign / SubAssign (例如: cf1 += cf2)
        impl $trait_assign<CashFlows> for CashFlows {
            fn $method_assign(&mut self, rhs: CashFlows) {
                for (date, value) in rhs.flows {
                    let entry = self.flows.entry(date).or_insert(0.0);
                    *entry $op value;
                }
            }
        }

        // 2. 實作 Add / Sub (例如: let cf3 = cf1 + cf2)
        // 直接利用已經寫好的 Assign 邏輯，避免重複程式碼
        impl $trait_op<CashFlows> for CashFlows {
            type Output = CashFlows;

            fn $method_op(mut self, rhs: CashFlows) -> Self::Output {
                self.$method_assign(rhs);
                self
            }
        }
    };
}

// 一口氣生成所有運算子實作
impl_cashflows_arithmetic!(Add, add, AddAssign, add_assign, +=);
impl_cashflows_arithmetic!(Sub, sub, SubAssign, sub_assign, -=);


impl MulAssign<f64> for CashFlows {
    fn mul_assign(&mut self, rhs: f64) {
        // 直接遍歷內部的 HashMap 修改 value
        for val in self.flows.values_mut() {
            *val *= rhs;
        }
    }
}

// 實作 let new_cf = cf * 1.5;
impl Mul<f64> for CashFlows {
    type Output = CashFlows;
    fn mul(mut self, rhs: f64) -> Self::Output {
        self *= rhs; // 複用 mul_assign 邏輯
        self
    }
}


impl Neg for CashFlows {
    type Output = CashFlows;

    fn neg(mut self) -> Self::Output {
        // 直接調用我們剛剛實作好的 MulAssign<f64>
        self *= -1.0; 
        self
    }
}

