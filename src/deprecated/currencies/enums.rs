use std::fmt;

use serde::{Deserialize, Serialize};

use super::traits::CurrencyDetails;
use crate::utils::errors::{AtlasError, Result};

/// # Self
/// Enum for currencies supported by the library
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Currency {
    /// US Dollar
    USD,
    /// Euro
    EUR,
    /// Japanese Yen
    JPY,
    /// South African Rand
    ZAR,
    /// Chilean Peso
    CLP,
    /// Chilean Unidad de Fomento
    CLF,
    /// Swiss Franc
    CHF,
    /// Brazilian Real
    BRL,
    /// Colombian Peso
    COP,
    /// Mexican Peso
    MXN,
    /// Australian Dollar
    AUD,
    /// Canadian Dollar
    CAD,
    /// Chinese Yuan
    CNY,
    /// British Pound
    GBP,
    /// New Zealand Dollar
    NZD,
    /// Norwegian Krone
    NOK,
    /// Swedish Krona
    SEK,
    /// Peruvian Sol
    PEN,
    /// Chinese Yuan (offshore)
    CNH,
    /// Indian Rupee
    INR,
    /// New Taiwan Dollar
    TWD,
    /// Hong Kong Dollar
    HKD,
    /// South Korean Won
    KRW,
    /// Danish Krone
    DKK,
    /// Indonesian Rupiah
    IDR,
}

impl Currency {
    /// Returns static metadata about the currency as
    /// (alphabetic code, display name, symbol, decimal precision, numeric ISO 4217 code).
    #[must_use]
    pub const fn details(self) -> (&'static str, &'static str, &'static str, u8, u16) {
        match self {
            Self::USD => ("USD", "US Dollar", "$", 2, 840),
            Self::EUR => ("EUR", "Euro", "€", 2, 978),
            Self::JPY => ("JPY", "Japanese Yen", "¥", 0, 392),
            Self::ZAR => ("ZAR", "South African Rand", "R", 2, 710),
            Self::CLP => ("CLP", "Chilean Peso", "$", 0, 152),
            Self::CLF => ("CLF", "Chilean Unidad de Fomento", "UF", 4, 990),
            Self::CHF => ("CHF", "Swiss Franc", "Fr", 2, 756),
            Self::BRL => ("BRL", "Brazilian Real", "R$", 2, 986),
            Self::COP => ("COP", "Colombian Peso", "$", 2, 170),
            Self::MXN => ("MXN", "Mexican Peso", "Mex$", 2, 484),
            Self::AUD => ("AUD", "Australian Dollar", "A$", 2, 36),
            Self::CAD => ("CAD", "Canadian Dollar", "Can$", 2, 124),
            Self::CNY => ("CNY", "Chinese Yuan", "¥", 2, 156),
            Self::GBP => ("GBP", "British Pound", "£", 2, 826),
            Self::NZD => ("NZD", "New Zealand Dollar", "NZ$", 2, 554),
            Self::NOK => ("NOK", "Norwegian Krone", "kr", 2, 578),
            Self::SEK => ("SEK", "Swedish Krona", "kr", 2, 752),
            Self::PEN => ("PEN", "Peruvian Sol", "S/.", 2, 604),
            Self::CNH => ("CNH", "Chinese Yuan (offshore)", "¥", 2, 156),
            Self::INR => ("INR", "Indian Rupee", "₹", 2, 356),
            Self::TWD => ("TWD", "New Taiwan Dollar", "NT$", 2, 901),
            Self::HKD => ("HKD", "Hong Kong Dollar", "HK$", 2, 344),
            Self::KRW => ("KRW", "South Korean Won", "₩", 0, 410),
            Self::DKK => ("DKK", "Danish Krone", "kr", 2, 208),
            Self::IDR => ("IDR", "Indonesian Rupiah", "Rp", 2, 360),
        }
    }

    /// Returns the alphabetic code of the currency.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.details().0
    }
    /// Returns the display name of the currency.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.details().1
    }
    /// Returns the symbol of the currency.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        self.details().2
    }
    /// Returns the decimal precision of the currency.
    #[must_use]
    pub const fn precision(self) -> u8 {
        self.details().3
    }
    /// Returns the numeric ISO 4217 code of the currency.
    #[must_use]
    pub const fn numeric_code(self) -> u16 {
        self.details().4
    }
}

impl CurrencyDetails for Currency {
    fn code(&self) -> &'static str {
        self.as_str()
    }
    fn name(&self) -> &'static str {
        self.details().1
    }
    fn symbol(&self) -> &'static str {
        self.details().2
    }
    fn precision(&self) -> u8 {
        self.details().3
    }
    fn numeric_code(&self) -> u16 {
        self.details().4
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for Currency {
    type Error = AtlasError;

    fn try_from(s: &str) -> Result<Self> {
        // trim white space
        let s = s.trim();
        match s {
            "USD" => Ok(Self::USD),
            "EUR" => Ok(Self::EUR),
            "JPY" => Ok(Self::JPY),
            "ZAR" => Ok(Self::ZAR),
            "CLP" => Ok(Self::CLP),
            "CLF" => Ok(Self::CLF),
            "CHF" => Ok(Self::CHF),
            "BRL" => Ok(Self::BRL),
            "COP" => Ok(Self::COP),
            "MXN" => Ok(Self::MXN),
            "AUD" => Ok(Self::AUD),
            "CAD" => Ok(Self::CAD),
            "CNY" => Ok(Self::CNY),
            "GBP" => Ok(Self::GBP),
            "NZD" => Ok(Self::NZD),
            "NOK" => Ok(Self::NOK),
            "SEK" => Ok(Self::SEK),
            "PEN" => Ok(Self::PEN),
            "CNH" => Ok(Self::CNH),
            "INR" => Ok(Self::INR),
            "TWD" => Ok(Self::TWD),
            "HKD" => Ok(Self::HKD),
            "KRW" => Ok(Self::KRW),
            "DKK" => Ok(Self::DKK),
            "IDR" => Ok(Self::IDR),
            _ => Err(AtlasError::InvalidValueErr(format!(
                "Invalid currency: {s}"
            ))),
        }
    }
}

impl TryFrom<String> for Currency {
    type Error = AtlasError;
    fn try_from(s: String) -> Result<Self> {
        Self::try_from(s.as_str())
    }
}

impl std::str::FromStr for Currency {
    type Err = AtlasError;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

impl From<Currency> for String {
    fn from(c: Currency) -> Self {
        c.as_str().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const ALL: &[Currency] = &[
        Currency::USD,
        Currency::EUR,
        Currency::JPY,
        Currency::ZAR,
        Currency::CLP,
        Currency::CLF,
        Currency::CHF,
        Currency::BRL,
        Currency::COP,
        Currency::MXN,
        Currency::AUD,
        Currency::CAD,
        Currency::CNY,
        Currency::GBP,
        Currency::NZD,
        Currency::NOK,
        Currency::SEK,
        Currency::PEN,
        Currency::CNH,
        Currency::INR,
        Currency::TWD,
        Currency::HKD,
        Currency::KRW,
        Currency::DKK,
        Currency::IDR,
    ];

    #[test]
    fn details_are_self_consistent_for_all_currencies() {
        for &c in ALL {
            let (code, name, symbol, precision, numeric_code) = c.details();

            assert_eq!(c.as_str(), code);
            assert_eq!(c.name(), name);
            assert_eq!(c.symbol(), symbol);
            assert_eq!(c.precision(), precision);
            assert_eq!(c.numeric_code(), numeric_code);

            assert_eq!(CurrencyDetails::code(&c), code);
            assert_eq!(CurrencyDetails::name(&c), name);
            assert_eq!(CurrencyDetails::symbol(&c), symbol);
            assert_eq!(CurrencyDetails::precision(&c), precision);
            assert_eq!(CurrencyDetails::numeric_code(&c), numeric_code);

            assert_eq!(c.to_string(), code);

            let s: String = c.into();
            assert_eq!(s, code);
        }
    }

    #[test]
    fn try_from_str_parses_known_codes_and_trims() -> Result<()> {
        assert_eq!(Currency::try_from("USD")?, Currency::USD);
        assert_eq!(Currency::try_from("  USD ")?, Currency::USD);
        assert_eq!(Currency::try_from("\nEUR\t")?, Currency::EUR);
        Ok(())
    }

    #[test]
    fn try_from_string_parses_same_as_str() -> Result<()> {
        let c = Currency::try_from("JPY".to_string())?;
        assert_eq!(c, Currency::JPY);
        Ok(())
    }

    #[test]
    fn from_str_parses_same_as_try_from() -> Result<()> {
        let c = Currency::from_str("GBP")?;
        assert_eq!(c, Currency::GBP);
        Ok(())
    }

    #[test]
    fn invalid_currency_rejected() {
        assert!(Currency::try_from("NOPE").is_err());
        assert!(Currency::from_str("usd").is_err());
        assert!(Currency::try_from("").is_err());
    }

    #[test]
    fn spot_checks_for_non_trivial_metadata() {
        // Precision edge cases
        assert_eq!(Currency::JPY.precision(), 0);
        assert_eq!(Currency::CLF.precision(), 4);

        assert_eq!(Currency::USD.numeric_code(), 840);
        assert_eq!(Currency::EUR.numeric_code(), 978);
        assert_eq!(Currency::KRW.numeric_code(), 410);

        assert_eq!(Currency::EUR.symbol(), "€");
        assert_eq!(Currency::KRW.symbol(), "₩");

        assert_eq!(Currency::CNY.numeric_code(), 156);
        assert_eq!(Currency::CNH.numeric_code(), 156);
    }
}
