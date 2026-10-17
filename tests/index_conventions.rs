//! Rate metadata and index integration checks.
use quantsupport::prelude::*;

#[test]
fn indices_use_local_holidays_and_term_fixing_conventions() {
    for (index, calendar) in [
        (MarketIndex::AONIA, "Australia(Settlement)"),
        (MarketIndex::CORRA, "Canada(Settlement)"),
        (MarketIndex::SARON, "Switzerland"),
        (MarketIndex::NZONIA, "NewZealand(Wellington)"),
        (MarketIndex::NOWA, "Norway"),
        (MarketIndex::SWESTR, "Sweden"),
        (MarketIndex::TIBOR3m, "Japan"),
        (MarketIndex::FedFunds, "UnitedStates(FederalReserve)"),
        (MarketIndex::CDI, "Brazil(Settlement)"),
        (MarketIndex::BBSW3m, "Australia(Settlement)"),
        (MarketIndex::BKBM3m, "NewZealand(Wellington)"),
        (MarketIndex::NIBOR3m, "Norway"),
        (MarketIndex::DESTR, "Denmark"),
        (MarketIndex::KOFR, "SouthKorea(Settlement)"),
        (MarketIndex::ZARONIA, "SouthAfrica"),
        (MarketIndex::SHIBOR3m, "China(IB)"),
    ] {
        assert_eq!(
            index.rate_index_details().unwrap().calendar().impl_name(),
            calendar
        );
        assert_eq!(index.to_string().parse::<MarketIndex>().unwrap(), index);
        assert_eq!(
            serde_json::from_str::<MarketIndex>(&serde_json::to_string(&index).unwrap()).unwrap(),
            index
        );
    }
    let overnight_shibor = MarketIndex::SHIBOR1d.rate_index_details().unwrap();
    assert_eq!(overnight_shibor.fixing_lag(), 0);
    assert!(!overnight_shibor.is_in_arrears());
    assert_eq!(
        MarketIndex::SHIBOR1w
            .rate_index_details()
            .unwrap()
            .fixing_lag(),
        1
    );
    assert_eq!(
        MarketIndex::NIBOR3m
            .rate_index_details()
            .unwrap()
            .fixing_lag(),
        2
    );
    assert_eq!(
        MarketIndex::EURIBOR1w
            .rate_index_details()
            .unwrap()
            .frequency(),
        Frequency::Weekly
    );
    assert_eq!(
        MarketIndex::CDI
            .rate_index_details()
            .unwrap()
            .rate_definition(),
        RateDefinition::new(
            DayCounter::Business252,
            Compounding::Compounded,
            Frequency::Annual
        )
    );
}

#[test]
fn overnight_builders_accept_effective_annual_cdi_and_reject_term_indices() {
    for index in [
        MarketIndex::CDI,
        MarketIndex::FedFunds,
        MarketIndex::DESTR,
        MarketIndex::KOFR,
        MarketIndex::ZARONIA,
    ] {
        let ois = MakeOvernightIndexSwap::<f64>::new(
            index,
            Date::new(2026, 1, 15),
            Period::new(1, TimeUnit::Years),
            0.04,
        )
        .build()
        .unwrap();
        assert!(!ois.floating_leg().cashflows().is_empty());
    }
    assert!(MakeOvernightIndexSwap::<f64>::new(
        MarketIndex::SHIBOR1d,
        Date::new(2026, 1, 15),
        Period::new(1, TimeUnit::Years),
        0.04
    )
    .build()
    .is_err());
}
