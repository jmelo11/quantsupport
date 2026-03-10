use super::date::Date;
use super::enums::Weekday;

/// Represents IMM (International Monetary Market) dates and codes for futures contracts.
pub struct IMM {}

impl IMM {
    /// Checks if a given date is an IMM date (third Wednesday of the month).
    ///
    /// ## Parameters
    /// * `date` - The date to check
    /// * `main_cycle` - If true, only checks main cycle months (3, 6, 9, 12)
    #[must_use]
    pub fn is_imm_date(date: Date, main_cycle: bool) -> bool {
        if date.weekday() != Weekday::Wednesday {
            return false;
        }
        if date.day() < 15 || date.day() > 21 {
            return false;
        }
        if !main_cycle {
            return true;
        }
        matches!(date.month(), 3 | 6 | 9 | 12)
    }

    /// Validates if a string is a valid IMM code.
    ///
    /// # Parameters
    /// * `in_` - The code string to validate (e.g., "F3")
    /// * `main_cycle` - If true, only validates main cycle codes
    #[must_use]
    pub fn is_imm_code(in_: &str, main_cycle: bool) -> bool {
        if in_.len() != 2 {
            return false;
        }
        let str1 = "0123456789";
        let loc = str1.find(&in_[1..2]);
        if loc.is_none() {
            return false;
        }
        let str1 = if main_cycle {
            "hmzuHMZU"
        } else {
            "fghjkmnquvxzFGHJKMNQUVXZ"
        };
        let loc = str1.find(&in_[0..1]);
        loc.is_some()
    }

    /// Returns the IMM code for a given IMM date.
    ///
    /// # Parameters
    /// * `imm_date` - An IMM date to convert to code
    ///
    /// # Panics
    /// Panics if the date is not a valid IMM date
    #[must_use]
    pub fn code(imm_date: Date) -> String {
        assert!(
            Self::is_imm_date(imm_date, false),
            "{imm_date} is not an IMM date"
        );
        let y = imm_date.year() % 10;
        match imm_date.month() {
            1 => format!("F{y}"),
            2 => format!("G{y}"),
            3 => format!("H{y}"),
            4 => format!("J{y}"),
            5 => format!("K{y}"),
            6 => format!("M{y}"),
            7 => format!("N{y}"),
            8 => format!("Q{y}"),
            9 => format!("U{y}"),
            10 => format!("V{y}"),
            11 => format!("X{y}"),
            12 => format!("Z{y}"),
            _ => panic!("Invalid month number"),
        }
    }

    /// Converts an IMM code to the corresponding date using a reference date.
    ///
    /// ## Parameters
    /// * `imm_code` - The IMM code to convert (e.g., "F3")
    /// * `reference_date` - A reference date to determine the correct year
    ///
    /// ## Panics
    /// Panics if the reference date is empty or the code is invalid
    #[must_use]
    pub fn date(imm_code: &str, reference_date: Date) -> Date {
        assert!(
            reference_date != Date::empty(),
            "No reference date provided"
        );

        let code = imm_code.to_uppercase();
        let ms = &code[0..1];
        let m = match ms {
            "F" => 1,
            "G" => 2,
            "H" => 3,
            "J" => 4,
            "K" => 5,
            "M" => 6,
            "N" => 7,
            "Q" => 8,
            "U" => 9,
            "V" => 10,
            "X" => 11,
            "Z" => 12,
            _ => panic!("Invalid IMM month letter"),
        };
        let mut y = match code[1..2].parse::<i32>() {
            Ok(n) => n,
            Err(e) => panic!("Invalid IMM year number: {e}"),
        };
        if y == 0 && reference_date.year() <= 1909 {
            y += 10;
        }
        let reference_year = reference_date.year() % 10;
        y += reference_date.year() - reference_year;

        let result = Self::next_date(Date::new(y, m, 1), false);
        if result < reference_date {
            return Self::next_date(Date::new(y + 10, m, 1), false);
        }
        result
    }

    /// Finds the next IMM date after the given reference date.
    ///
    /// # Arguments
    /// * `reference_date` - The starting date for the search
    /// * `main_cycle` - If true, only finds dates in main cycle months (3, 6, 9, 12)
    ///
    /// # Panics
    /// Panics if the reference date is empty
    #[must_use]
    pub fn next_date(reference_date: Date, main_cycle: bool) -> Date {
        assert!(
            reference_date != Date::empty(),
            "No reference date provided"
        );
        let y = reference_date.year();
        let m = reference_date.month();
        let offset = if main_cycle { 3 } else { 1 };
        let skip_months = offset - (m % offset);
        let mut m = if skip_months != offset || reference_date.day() > 21 {
            skip_months + m
        } else {
            m
        };
        let mut y = y;
        if m > 12 {
            m -= 12;
            y += 1;
        }
        let result = Date::nth_weekday(3, Weekday::Wednesday, m, y);
        if result <= reference_date {
            return Self::next_date(Date::new(y, m, 22), main_cycle);
        }
        result
    }

    /// Finds the next IMM date after a given IMM code.
    ///
    /// # Arguments
    /// * `imm_code` - The IMM code to start from
    /// * `main_cycle` - If true, only finds dates in main cycle months
    /// * `reference_date` - A reference date to resolve the code
    #[must_use]
    pub fn next_date_with_code(imm_code: &str, main_cycle: bool, reference_date: Date) -> Date {
        let imm_date = Self::date(imm_code, reference_date);
        Self::next_date(imm_date + 1, main_cycle)
    }

    /// Returns the IMM code for the next IMM date after the given date.
    ///
    /// # Arguments
    /// * `d` - The reference date
    /// * `main_cycle` - If true, only considers main cycle months
    #[must_use]
    pub fn next_code(d: Date, main_cycle: bool) -> String {
        let next = Self::next_date(d, main_cycle);
        Self::code(next)
    }

    /// Returns the IMM code for the next IMM date after a given IMM code.
    ///
    /// # Arguments
    /// * `imm_code` - The current IMM code
    /// * `main_cycle` - If true, only considers main cycle months
    /// * `reference_date` - A reference date to resolve the code
    #[must_use]
    pub fn next_code_with_code(imm_code: &str, main_cycle: bool, reference_date: Date) -> String {
        let imm_date = Self::date(imm_code, reference_date);
        let next = Self::next_date(imm_date, main_cycle);
        Self::code(next)
    }
}

#[cfg(test)]
mod tests {
    use super::super::date::Date;
    use super::IMM;

    #[test]
    fn test_is_imm_date() {
        let d = Date::new(2023, 8, 1);
        assert!(!IMM::is_imm_date(d, false));

        let d = Date::new(2023, 8, 16);
        assert!(IMM::is_imm_date(d, false));

        let d = Date::new(2023, 8, 16);
        assert!(!IMM::is_imm_date(d, true));

        let d = Date::new(2023, 9, 20);
        assert!(IMM::is_imm_date(d, true));
        assert!(IMM::is_imm_date(d, false));
    }

    #[test]
    fn test_is_imm_code() {
        let s = "F3".to_string();
        assert!(IMM::is_imm_code(&s, false));

        let s = "F3".to_string();
        assert!(!IMM::is_imm_code(&s, true));

        let s = "Z3".to_string();
        assert!(IMM::is_imm_code(&s, true));

        let s = "Z3".to_string();
        assert!(IMM::is_imm_code(&s, false));
    }

    #[test]
    fn test_code() {
        let d = Date::new(2023, 8, 16);
        assert_eq!(IMM::code(d), "Q3");

        let d = Date::new(2023, 9, 20);
        assert_eq!(IMM::code(d), "U3");
    }

    #[test]
    fn test_date() {
        let d = Date::new(2023, 8, 16);
        let code1 = "Q3".to_string();
        assert_eq!(IMM::date(&code1, d), d);

        let code2 = "U3".to_string();
        assert_eq!(IMM::date(&code2, d), Date::new(2023, 9, 20));
    }

    #[test]
    fn test_next_date() {
        let d = Date::new(2023, 8, 16);
        assert_eq!(IMM::next_date(d, false), Date::new(2023, 9, 20));

        let d = Date::new(2023, 8, 16);
        assert_eq!(IMM::next_date(d, true), Date::new(2023, 9, 20));

        let d = Date::new(2023, 9, 20);
        assert_eq!(IMM::next_date(d, false), Date::new(2023, 10, 18));

        let d = Date::new(2023, 9, 20);
        assert_eq!(IMM::next_date(d, true), Date::new(2023, 12, 20));
    }

    #[test]
    fn test_next_date_with_code() {
        let d = Date::new(2023, 8, 16);
        let code = "Q3".to_string();
        assert_eq!(
            IMM::next_date_with_code(&code, false, d),
            Date::new(2023, 9, 20)
        );

        let d = Date::new(2023, 8, 16);
        let code = "U3".to_string();
        assert_eq!(
            IMM::next_date_with_code(&code, false, d),
            Date::new(2023, 10, 18)
        );

        let d = Date::new(2023, 8, 16);
        let code = "U3".to_string();
        assert_eq!(
            IMM::next_date_with_code(&code, true, d),
            Date::new(2023, 12, 20)
        );
    }

    #[test]
    fn test_next_code() {
        let d = Date::new(2023, 8, 16);
        assert_eq!(IMM::next_code(d, false), "U3");

        let d = Date::new(2023, 8, 16);
        assert_eq!(IMM::next_code(d, true), "U3");

        let d = Date::new(2023, 9, 20);
        assert_eq!(IMM::next_code(d, false), "V3");

        let d = Date::new(2023, 9, 20);
        assert_eq!(IMM::next_code(d, true), "Z3");
    }

    #[test]
    fn test_next_code_with_code() {
        let d = Date::new(2023, 8, 16);
        let code = "Q3".to_string();
        assert_eq!(IMM::next_code_with_code(&code, false, d), "U3");

        let d = Date::new(2023, 8, 16);
        let code = "U3".to_string();
        assert_eq!(IMM::next_code_with_code(&code, false, d), "V3");

        let d = Date::new(2023, 8, 16);
        let code = "U3".to_string();
        assert_eq!(IMM::next_code_with_code(&code, true, d), "Z3");
    }
}
