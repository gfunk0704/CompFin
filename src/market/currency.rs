
#[derive(Clone)]
pub struct Currency {
    code: String,
    digits: u32
}

impl Currency {
    pub fn new(code: String, digits: u32) -> Currency {
        Currency { code: code, digits: digits }
    } 

    pub fn code(&self) -> String {
        self.code.clone()
    }

    pub fn digits(&self) -> u32 {
        self.digits
    }
}

#[derive(Clone)]
pub struct CurrencyPair {
    ccy1: Currency,
    ccy2: Currency
}

impl CurrencyPair {
    pub fn new(ccy1: Currency, ccy2: Currency) -> CurrencyPair {
        CurrencyPair { ccy1: ccy1, ccy2: ccy2 }
    }

    pub fn ccy1(&self) -> &Currency {
        &self.ccy1
    }

    pub fn ccy2(&self) -> &Currency {
        &self.ccy2
    }
}

