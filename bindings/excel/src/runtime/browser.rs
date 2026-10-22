//! Read-only, owned snapshots for the ribbon companion. No AD objects cross the ABI.

use super::registry::QsObject;
use quantsupport::prelude::{Instrument, Trade};
use serde::Serialize;

#[derive(Default, Serialize)]
pub struct BrowserSnapshot {
    pub workbook: String,
    pub objects: Vec<BrowserObject>,
    pub error: String,
}

#[derive(Clone, Serialize)]
pub struct BrowserLocation {
    pub sheet_id: u64,
    pub sheet: String,
    /// One-based Excel coordinates.
    pub row: i32,
    pub column: i32,
    pub formula: String,
}

#[derive(Default, Serialize)]
pub struct BrowserObject {
    pub name: String,
    pub handle: String,
    pub kind: String,
    pub family: String,
    pub revision: u64,
    pub current_inputs: bool,
    pub evaluation_date: String,
    pub snapshot_timestamp: String,
    pub quote_level: String,
    pub details: Vec<BrowserDetail>,
    pub locations: Vec<BrowserLocation>,
    pub dependencies: Vec<String>,
}

#[derive(Serialize)]
pub struct BrowserDetail {
    pub label: String,
    pub value: String,
}

impl BrowserObject {
    fn detail(&mut self, label: &str, value: impl ToString) {
        self.details.push(BrowserDetail {
            label: label.into(),
            value: value.to_string(),
        });
    }

    pub fn from_object(object: &QsObject) -> Self {
        let mut result = Self {
            kind: object.kind().into(),
            ..Self::default()
        };
        result.family = match object {
            QsObject::PricingContext(_) => "Pricing contexts",
            QsObject::Instrument(_) => "Instruments",
            QsObject::EvaluationResults(_) => "Results",
            QsObject::Pricer(_) | QsObject::DiscountPolicy(_) => "Pricing setup",
            _ if object.kind().ends_with("Trade") => "Trades",
            _ => "Market & configuration",
        }
        .into();
        match object {
            QsObject::PricingContext(context) => {
                result.evaluation_date = context.evaluation_date().to_string();
                result.snapshot_timestamp = context
                    .snapshot_timestamp()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                result.quote_level = format!("{:?}", context.quote_level());
                result.detail("Base currency", context.base_currency());
                result.detail("Base index", context.base_index());
                let elements = context.constructed_elements();
                let sorted = |mut values: Vec<String>| {
                    values.sort();
                    values.join(", ")
                };
                result.detail(
                    "Constructed curves",
                    sorted(
                        elements
                            .discount_curves()
                            .keys()
                            .map(ToString::to_string)
                            .collect(),
                    ),
                );
                result.detail(
                    "Volatility surfaces",
                    sorted(
                        elements
                            .volatility_surfaces()
                            .keys()
                            .map(ToString::to_string)
                            .collect(),
                    ),
                );
                result.detail(
                    "Volatility cubes",
                    sorted(
                        elements
                            .volatility_cubes()
                            .keys()
                            .map(ToString::to_string)
                            .collect(),
                    ),
                );
                result.detail(
                    "Simulations",
                    sorted(
                        elements
                            .simulations()
                            .keys()
                            .map(ToString::to_string)
                            .collect(),
                    ),
                );
            }
            QsObject::Instrument(instrument) => {
                use super::products::NativeInstrument;
                macro_rules! identifier {
                    ($($variant:ident),* $(,)?) => { match &instrument.native {
                        $(NativeInstrument::$variant(value) => value.identifier(),)*
                    } };
                }
                result.detail(
                    "Identifier",
                    identifier!(
                        Loan,
                        Swap,
                        BasisSwap,
                        FloatFloatCrossCurrencySwap,
                        FixFloatCrossCurrencySwap,
                        FixedRateBond,
                        FixedRateDeposit,
                        FloatingRateNote,
                        FxForward,
                        FxEuropeanOption,
                        EquityEuropeanOption,
                        EquityForward,
                        CreditDefaultSwap,
                        CapFloor,
                        CapletFloorlet,
                        RateFutures,
                        EuropeanSwaption
                    ),
                );
                result.detail("Notional", instrument.notional);
                if let Some(value) = instrument.foreign_notional {
                    result.detail("Foreign notional", value);
                }
            }
            QsObject::QuoteStore(store) => result.detail("Reference date", store.reference_date()),
            _ => {}
        }
        macro_rules! trade_details {
            ($($variant:ident),* $(,)?) => { match object {
                $(QsObject::$variant(trade) => {
                    result.detail("Identifier", trade.instrument().identifier());
                    result.detail("Trade date", trade.trade_date());
                    result.detail("Side", format!("{:?}", trade.side()));
                },)*
                _ => {}
            } };
        }
        trade_details!(
            LoanTrade,
            FxForwardTrade,
            SwapTrade,
            BasisSwapTrade,
            FloatFloatCrossCurrencySwapTrade,
            FixFloatCrossCurrencySwapTrade,
            FixedRateBondTrade,
            FixedRateDepositTrade,
            FloatingRateNoteTrade,
            FxEuropeanOptionTrade,
            EquityEuropeanOptionTrade,
            EquityForwardTrade,
            EuropeanSwaptionTrade,
            CdsTrade,
            CapFloorTrade,
            CapletFloorletTrade,
            RateFuturesTrade
        );
        result
    }
}

#[cfg(windows)]
pub mod windows {
    use super::BrowserSnapshot;
    use std::{
        ffi::{c_char, CString},
        sync::atomic::{AtomicU32, Ordering},
    };
    use windows::Win32::System::Threading::GetCurrentThreadId;

    static MAIN_THREAD: AtomicU32 = AtomicU32::new(0);

    pub fn start() {
        MAIN_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::Release);
    }
    pub fn stop() {
        MAIN_THREAD.store(0, Ordering::Release);
    }

    #[no_mangle]
    pub extern "system" fn qs_browser_api_version() -> u32 {
        1
    }

    /// Returns an owned UTF-8 JSON string, released by qs_browser_free.
    ///
    /// # Safety
    /// `workbook` must be null or point to a valid UTF-16 buffer containing at
    /// least `length` code units. The buffer remains owned by the caller.
    #[no_mangle]
    pub unsafe extern "system" fn qs_browser_snapshot(
        workbook: *const u16,
        length: usize,
    ) -> *mut c_char {
        let result = std::panic::catch_unwind(|| {
            if MAIN_THREAD.load(Ordering::Acquire) != unsafe { GetCurrentThreadId() } {
                return BrowserSnapshot { error: "Open the native QuantSupport add-in and inspect objects on Excel's main thread.".into(), ..BrowserSnapshot::default() };
            }
            if workbook.is_null() || length == 0 || length > 32767 {
                return BrowserSnapshot { error: "Select an open workbook.".into(), ..BrowserSnapshot::default() };
            }
            let name = String::from_utf16(unsafe { std::slice::from_raw_parts(workbook, length) });
            match name {
                Ok(name) => super::super::lifecycle::browser_snapshot(&name),
                Err(_) => BrowserSnapshot { error: "Invalid workbook name.".into(), ..BrowserSnapshot::default() },
            }
        }).unwrap_or_else(|_| BrowserSnapshot { error: "Object inspection is temporarily unavailable; refresh when Excel is ready.".into(), ..BrowserSnapshot::default() });
        let json = serde_json::to_string(&result)
            .unwrap_or_else(|_| "{\"error\":\"Could not serialize the object browser.\"}".into());
        CString::new(json)
            .expect("JSON escapes NUL characters")
            .into_raw()
    }

    /// # Safety
    /// `value` must be null or an unfreed pointer returned by qs_browser_snapshot.
    #[no_mangle]
    pub unsafe extern "system" fn qs_browser_free(value: *mut c_char) {
        if !value.is_null() {
            drop(unsafe { CString::from_raw(value) });
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn read(name: &[u16]) -> serde_json::Value {
            let value = unsafe { qs_browser_snapshot(name.as_ptr(), name.len()) };
            let json = unsafe { std::ffi::CStr::from_ptr(value) }.to_str().unwrap();
            let result = serde_json::from_str(json).unwrap();
            unsafe {
                qs_browser_free(value);
            }
            result
        }

        #[test]
        fn abi_rejects_background_calls_and_invalid_utf16_and_releases_its_buffers() {
            start();
            let name: Vec<u16> = "Café.xlsx".encode_utf16().collect();
            let snapshot = read(&name);
            assert_eq!(snapshot["workbook"], "Café.xlsx");
            assert_eq!(snapshot["error"], "");
            assert_eq!(snapshot["objects"], serde_json::json!([]));
            assert!(std::thread::spawn(move || read(&name)["error"]
                .as_str()
                .unwrap()
                .contains("main thread"))
            .join()
            .unwrap());
            assert_eq!(read(&[0xd800])["error"], "Invalid workbook name.");
            assert_eq!(read(&[])["error"], "Select an open workbook.");
            stop();
            assert!(read(&[65])["error"]
                .as_str()
                .unwrap()
                .contains("main thread"));
            unsafe {
                qs_browser_free(std::ptr::null_mut());
            }
        }
    }
}
