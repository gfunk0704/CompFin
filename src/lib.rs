pub mod configuration;

pub mod instrument {
    pub mod instrument;
    pub mod nominalgenerator;

    pub mod interestrate {
        pub mod flowobserver;
    }

    pub mod leg {
        pub mod legcharacters;
        pub mod fixedratelegcharacters;
        pub mod floatingratelegcharacters;

        pub mod fixingratecalculator {
            pub mod fixingratecalculator;
            pub mod termratecalculator;
            pub mod compoundingrateindexcalculator;
        }
    }
}

pub mod interestrate {
    pub mod compounding;
    pub mod index {
        pub mod cachebackend;
        pub mod interestrateindex;
        pub mod termrateindex;
        pub mod compoundingrateindex;
        pub mod interestrateindexmanager;
        pub mod cachedinterestrateindex;
        pub mod compoundingconvention;
    }
}

pub mod manager {
    pub mod namedobject;
    pub mod managererror;
    pub mod manager;
}

pub mod market {
    pub mod currency;
    pub mod market;
    pub mod singlecurrencymarket;
    pub mod fxmarket;
}

pub mod math {
    pub mod curve {
        pub mod curve;
        pub mod nonparametriccurve {
            pub mod nonparametriccurve;
            pub mod piecewisepolynomial;
            pub mod lagrangepolynomial;
        }
    }
    pub mod round;
}

pub mod model {
    pub mod interestrate {
        pub mod interestratecurve;
        pub mod precomputeddiscountcurve;
    }
}

pub mod objectwithuuid;

pub mod pricingcondition;

pub mod time {
    pub mod utility;
    pub mod period;
    pub mod rangeofdates;
    pub mod businessdayadjuster;
    pub mod optiondategenerator;

    pub mod recurringholiday {
        pub mod recurringholiday;
        pub mod weekendadjustment;
        pub mod fixeddateholiday;
        pub mod nthweekdayholiday;
        pub mod lastweekdayholiday;
        pub mod easterrelatedholiday;
    }

    pub mod calendar {
        pub mod holidaycalendar;
        pub mod simplecalendar;
        pub mod precomputedsimplecalendar;
        pub mod jointcalendar;
        pub mod holidaycalendarmanager;
    }

    pub mod schedule {
        pub mod scheduleperiod;
        pub mod generationdirection;
        pub mod calculationperiodgenerator;
        pub mod relativedategenerator;
        pub mod schedule;
        pub mod stubadjuster;
        pub mod schedulegeneratormanager;
    }

    pub mod daycounter {
        pub mod daycounter;
        pub mod constdaycounterdominator;
        pub mod isdaactualdaycounterdominator;
        pub mod icmaactualdaycountdominator;
        pub mod daycountergeneratormanager;
        pub mod numerator {
            pub mod actualnumerator;
            pub mod noleapnumerator;
            pub mod onenumerator;
            pub mod thirtynumerator;
        }
    }
}

pub mod value {
    pub mod cashflows;
    pub mod npv;
}







