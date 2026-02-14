use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum Compounding {
    Simple,
    Continuous,
    Annual, 
    Semiannual,
    Quarterly,
    Bimonthly,
    Monthly,
    Biweekly,
    Weekly,
    Daily
}

impl Compounding {
    fn get_frequency(&self) -> f64 {
        match self {
            Compounding::Annual => 1.0,
            Compounding::Semiannual => 2.0,
            Compounding::Quarterly => 4.0,
            Compounding::Bimonthly => 6.0,
            Compounding::Monthly => 12.0,
            Compounding::Biweekly => 26.0,
            Compounding::Weekly => 52.0,
            Compounding::Daily => 365.0,
            _ => 0.0
        }
    }

    pub fn future_value(&self, rate: f64, tau: f64) -> f64 {
        match self {
            Compounding::Simple => 1.0 + rate * tau,
            Compounding::Continuous => (rate * tau).exp(),
            _ => {
                let freq = self.get_frequency();
                (1.0 + rate / freq).powf(tau * freq)
            }
        }
    }

    pub fn implied_rate(&self, future_value: f64, tau: f64) -> f64 {
        match self {
            Compounding::Simple => (future_value - 1.0) / tau,
            Compounding::Continuous => future_value.ln() / tau,
            _ => {
                let freq = self.get_frequency();
                (future_value.powf(1.0 / (tau * freq)) - 1.0) * freq
            }
        }
    }
}