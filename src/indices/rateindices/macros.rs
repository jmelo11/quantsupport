// All index modules import the same types used by this shared declaration.
macro_rules! rate_index {
    (
        $(#[$meta:meta])*
        $name:ident {
            market_index: $variant:ident,
            currency: $currency:ident,
            calendar: $calendar:expr,
            fixing_lag: $fixing_lag:expr,
            day_counter: $day_counter:ident,
            compounding: $compounding:ident,
            compounding_frequency: $compounding_frequency:ident,
            frequency: $frequency:ident,
            is_in_arrears: $is_in_arrears:expr,
            swap_conventions: {
                fixed_day_counter: $fixed_day_counter:ident,
                fixed_frequency: $fixed_frequency:expr,
                floating_frequency: $floating_frequency:ident,
                settlement_days: $settlement_days:expr,
            },
        }
    ) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Serialize, Default, Deserialize)]
        pub struct $name;

        impl MarketIndexDetails for $name {
            fn quote_type(&self) -> QuoteType {
                QuoteType::Rate
            }

            fn name(&self) -> &'static str {
                stringify!($variant)
            }
        }

        impl RateIndexDetails for $name {
            fn calendar(&self) -> Calendar {
                $calendar
            }

            fn currency(&self) -> Currency {
                Currency::$currency
            }

            fn fixing_lag(&self) -> i64 {
                $fixing_lag
            }

            fn rate_definition(&self) -> RateDefinition {
                RateDefinition::new(
                    DayCounter::$day_counter,
                    Compounding::$compounding,
                    Frequency::$compounding_frequency,
                )
            }

            fn swap_conventions(&self, tenor: Period) -> SwapConventions {
                SwapConventions::new(
                    RateDefinition::new(
                        DayCounter::$fixed_day_counter,
                        Compounding::$compounding,
                        Frequency::Annual,
                    ),
                    ($fixed_frequency)(tenor),
                    Frequency::$floating_frequency,
                    DayCounter::$day_counter,
                    $settlement_days,
                    BusinessDayConvention::ModifiedFollowing,
                )
            }

            fn market_index(&self) -> MarketIndex {
                MarketIndex::$variant
            }

            fn frequency(&self) -> Frequency {
                Frequency::$frequency
            }

            fn is_in_arrears(&self) -> bool {
                $is_in_arrears
            }
        }
    };
}

pub(super) use rate_index;
