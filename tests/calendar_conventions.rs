//! QuantLib parity and integration checks for market calendars and indices.
use quantsupport::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Reference {
    calendars: Vec<CalendarReference>,
}
#[derive(Deserialize)]
struct CalendarReference {
    calendar: String,
    start_year: i32,
    end_year: i32,
    business_day_hash: String,
}

#[test]
fn calendars_match_pinned_quantlib_for_every_day() {
    let reference: Reference =
        serde_json::from_str(include_str!("data/quantlib_calendars.json")).unwrap();
    for row in reference.calendars {
        let calendar = Calendar::try_from(row.calendar.clone()).unwrap();
        let mut date = Date::new(row.start_year, 1, 1);
        let end = Date::new(row.end_year, 12, 31);
        let mut hash = 14_695_981_039_346_656_037_u64;
        while date <= end {
            hash ^= u64::from(calendar.is_business_day(&date));
            hash = hash.wrapping_mul(1_099_511_628_211);
            date += 1;
        }
        assert_eq!(
            format!("{hash:x}"),
            row.business_day_hash,
            "{} ({}–{})",
            row.calendar,
            row.start_year,
            row.end_year
        );
        let encoded = serde_json::to_string(&calendar).unwrap();
        assert_eq!(
            serde_json::from_str::<Calendar>(&encoded).unwrap(),
            calendar
        );
    }
}

#[test]
fn market_variants_and_working_weekends_affect_schedules() {
    let open = |calendar: &str, date| {
        Calendar::try_from(calendar.to_owned())
            .unwrap()
            .is_business_day(&date)
    };
    assert!(!open("Australia(Settlement)", Date::new(2026, 8, 3)));
    assert!(open("Australia(ASX)", Date::new(2026, 8, 3)));
    assert!(!open("Canada(Settlement)", Date::new(2026, 9, 30)));
    assert!(open("Canada(TSX)", Date::new(2026, 9, 30)));
    assert!(!open("NewZealand(Wellington)", Date::new(2026, 1, 19)));
    assert!(open("NewZealand(Auckland)", Date::new(2026, 1, 19)));
    assert!(!open("SouthKorea(KRX)", Date::new(2026, 12, 31)));
    assert!(open("SouthKorea(Settlement)", Date::new(2026, 12, 31)));
    let saturday = Date::new(2026, 2, 14);
    assert!(!open("China(SSE)", saturday));
    assert!(open("China(IB)", saturday));
    let mut interbank = Calendar::try_from("China(IB)".to_owned()).unwrap();
    assert_eq!(
        interbank.advance(saturday - 1, Period::new(1, TimeUnit::Days), None, false),
        saturday
    );
    interbank.add_holiday(saturday);
    assert!(!interbank.is_business_day(&saturday));
    interbank.remove_holiday(saturday);
    assert!(interbank.is_business_day(&saturday));
    let joint =
        Calendar::try_from("JointCalendar(JoinHolidays,Canada(Settlement),Switzerland)".to_owned())
            .unwrap();
    assert!(!joint.is_business_day(&Date::new(2026, 9, 30)));
}
