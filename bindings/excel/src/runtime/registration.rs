//! Excel registration constraints and checked native registration.

/// Excel's Function Wizard argument text is limited to 255 UTF-16 units.
/// Remove separator whitespace without changing argument names or positions.
pub fn argument_text(names: &str) -> Result<String, String> {
    let text = names
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(",");
    if text.encode_utf16().count() > 255 {
        return Err("argument names exceed Excel's 255-character registration limit".into());
    }
    Ok(text)
}

/// Typed callback result, separated from the Windows ABI for portable tests.
#[derive(Clone, Copy, Debug)]
pub enum RegistrationResult {
    Number(f64),
    Integer(i32),
    ExcelError(i32),
    OtherType(u32),
}

/// Validates callback success and Excel's numeric registration result.
///
/// Registration IDs are opaque tokens: their sign and magnitude do not indicate
/// success. In particular, zero and negative IDs must not be rejected.
/// https://learn.microsoft.com/en-us/office/client-developer/excel/xlfregister-form-1
pub fn checked_registration_id(status: i32, result: RegistrationResult) -> Result<f64, String> {
    if status != 0 {
        return Err(format!(
            "Excel callback failed with code {status}; result={result:?}"
        ));
    }
    let id = match result {
        RegistrationResult::Number(id) => id,
        RegistrationResult::Integer(id) => f64::from(id),
        RegistrationResult::ExcelError(code) => {
            return Err(format!(
                "Excel rejected registration (worksheet error {code})"
            ));
        }
        RegistrationResult::OtherType(kind) => {
            return Err(format!(
                "Excel returned a nonnumeric registration result (xltype=0x{kind:04X})"
            ));
        }
    };
    if !id.is_finite() {
        return Err(format!("Excel returned a non-finite registration ID: {id}"));
    }
    Ok(id)
}

#[cfg(windows)]
pub mod windows {
    use std::cell::RefCell;
    use xll_rs::{
        entrypoint::{excel12, excel_free, XLRET_SUCCESS},
        memory::xlAutoFree12,
        types::{
            XLBIT_XL_FREE, XLF_REGISTER, XLOPER12, XLTYPE_ERR, XLTYPE_INT, XLTYPE_NUM, XL_GET_NAME,
        },
    };

    const XLF_UNREGISTER: i32 = 201;
    const XLF_SET_NAME: i32 = 88;
    include!(concat!(env!("OUT_DIR"), "/worksheet_exports.rs"));
    thread_local! { static REGISTRATION_IDS: RefCell<Vec<(f64, &'static str)>> = const { RefCell::new(Vec::new()) }; }

    // Registration inputs belong to the DLL; callback results belong to Excel.
    struct Inputs(Vec<*mut XLOPER12>);
    impl Inputs {
        fn new(values: impl IntoIterator<Item = XLOPER12>) -> Self {
            Self(
                values
                    .into_iter()
                    .map(|value| Box::into_raw(Box::new(value)))
                    .collect(),
            )
        }
    }
    impl Drop for Inputs {
        fn drop(&mut self) {
            for value in self.0.drain(..) {
                xlAutoFree12(value);
            }
        }
    }

    fn free_result(result: &mut XLOPER12) {
        if result.xltype & XLBIT_XL_FREE != 0 {
            excel_free(result);
        }
    }

    fn registration_id(status: i32, result: &XLOPER12) -> Result<f64, String> {
        let value = match result.base_type() {
            XLTYPE_NUM => super::RegistrationResult::Number(unsafe { result.val.num }),
            XLTYPE_INT => super::RegistrationResult::Integer(unsafe { result.val.w }),
            XLTYPE_ERR => super::RegistrationResult::ExcelError(unsafe { result.val.err }),
            kind => super::RegistrationResult::OtherType(kind),
        };
        super::checked_registration_id(status, value)
    }

    pub fn register_functions() -> Result<(), String> {
        let functions = crate::worksheet_exports();
        if functions.is_empty() {
            return Err("QuantSupport has no worksheet exports".into());
        }
        let actual: std::collections::BTreeSet<_> = functions
            .iter()
            .flat_map(|function| {
                std::iter::once(function.name).chain(function.aliases.iter().copied())
            })
            .collect();
        let expected: std::collections::BTreeSet<_> =
            EXPECTED_WORKSHEET_EXPORTS.iter().copied().collect();
        if actual != expected {
            return Err(format!(
                "Worksheet export inventory mismatch: missing {:?}; unexpected {:?}",
                expected.difference(&actual).collect::<Vec<_>>(),
                actual.difference(&expected).collect::<Vec<_>>()
            ));
        }
        // Validate all metadata before registering any function.
        let argument_texts = functions
            .iter()
            .map(|function| {
                if crate::documentation::function_help::result_help(function.name).is_none() {
                    return Err(format!(
                        "{}: missing worksheet result/error documentation",
                        function.name
                    ));
                }
                super::argument_text(function.arg_names)
                    .map_err(|error| format!("{}: {error}", function.name))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (status, mut dll) = excel12(XL_GET_NAME, &mut []);
        let path = dll.as_string();
        free_result(&mut dll);
        let path = path
            .filter(|path| status == XLRET_SUCCESS && !path.is_empty())
            .ok_or("Excel did not supply the add-in path")?;
        for (function, arguments) in functions.iter().zip(argument_texts) {
            let generated = crate::documentation::function_help::argument_help(
                function.name,
                function.arg_names,
            );
            let help = if function.arg_help.is_empty() {
                generated.as_slice()
            } else {
                function.arg_help
            };
            for name in std::iter::once(function.name).chain(function.aliases.iter().copied()) {
                let mut values = vec![
                    XLOPER12::from_str(&path),
                    XLOPER12::from_str(function.rust_name),
                    XLOPER12::from_str(function.type_str),
                    XLOPER12::from_str(name),
                    XLOPER12::from_str(&arguments),
                    XLOPER12::from_int(1),
                    XLOPER12::from_str(function.category),
                    XLOPER12::missing(),
                    XLOPER12::missing(),
                    XLOPER12::from_str(function.help),
                ];
                values.extend(help.iter().map(|text| XLOPER12::from_str(text)));
                let mut inputs = Inputs::new(values);
                let (status, mut result) = excel12(XLF_REGISTER, &mut inputs.0);
                let id = registration_id(status, &result);
                free_result(&mut result);
                match id {
                    Ok(id) => REGISTRATION_IDS.with_borrow_mut(|ids| ids.push((id, name))),
                    Err(error) => {
                        unregister_functions();
                        return Err(format!(
                            "Could not register {name} ({}): {error}",
                            function.rust_name
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn unregister_functions() {
        // Release the registry borrow before callbacks, which may reenter the XLL.
        let registrations = REGISTRATION_IDS.with_borrow_mut(std::mem::take);
        for (id, name) in registrations.into_iter().rev() {
            // xlfUnregister alone leaves the hidden function-name definition.
            // https://learn.microsoft.com/en-us/office/client-developer/excel/xlfunregister-form-1
            let mut input = Inputs::new([XLOPER12::from_str(name)]);
            let (_, mut result) = excel12(XLF_SET_NAME, &mut input.0);
            free_result(&mut result);
            let mut id = XLOPER12::from_f64(id);
            let (_, mut result) = excel12(XLF_UNREGISTER, &mut [&mut id]);
            free_result(&mut result);
        }
    }

    pub fn show_error(message: &str) {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(
                window: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                flags: u32,
            ) -> i32;
        }
        let text: Vec<_> = format!("QuantSupport could not load.\n\n{message}")
            .encode_utf16()
            .chain([0])
            .collect();
        let caption: Vec<_> = "QuantSupport registration error"
            .encode_utf16()
            .chain([0])
            .collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn callback_success_with_excel_error_is_a_registration_failure() {
            let error = XLOPER12::from_err(xll_rs::types::XLERR_VALUE);
            assert!(registration_id(XLRET_SUCCESS, &error)
                .unwrap_err()
                .contains("rejected"));
            assert!(registration_id(32, &XLOPER12::from_f64(1.0)).is_err());
            for result in [
                XLOPER12::nil(),
                XLOPER12::from_f64(f64::NAN),
                XLOPER12::from_bool(true),
            ] {
                assert!(registration_id(XLRET_SUCCESS, &result).is_err());
            }
            for id in [42.0, 0.0, -42.0] {
                assert_eq!(
                    registration_id(XLRET_SUCCESS, &XLOPER12::from_f64(id)).unwrap(),
                    id
                );
            }
            assert_eq!(
                registration_id(XLRET_SUCCESS, &XLOPER12::from_int(-42)).unwrap(),
                -42.0
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{checked_registration_id, RegistrationResult};

    #[test]
    fn accepts_opaque_numeric_ids_regardless_of_sign_or_zero() {
        for id in [-2_147_483_648.0, -42.0, -0.0, 0.0, 42.0, 2_147_483_647.0] {
            assert_eq!(
                checked_registration_id(0, RegistrationResult::Number(id)).unwrap(),
                id
            );
        }
        for id in [i32::MIN, -42, 0, 42, i32::MAX] {
            assert_eq!(
                checked_registration_id(0, RegistrationResult::Integer(id)).unwrap(),
                f64::from(id)
            );
        }
    }

    #[test]
    fn still_rejects_failed_callbacks_excel_errors_and_nonnumeric_results() {
        assert!(
            checked_registration_id(32, RegistrationResult::Number(-42.0))
                .unwrap_err()
                .contains("code 32")
        );
        assert!(
            checked_registration_id(0, RegistrationResult::ExcelError(15))
                .unwrap_err()
                .contains("worksheet error 15")
        );
        // Boolean, string, missing, and nil results are not registration IDs.
        for kind in [0x0004, 0x0002, 0x0080, 0x0100] {
            let error =
                checked_registration_id(0, RegistrationResult::OtherType(kind)).unwrap_err();
            assert!(error.contains(&format!("xltype=0x{kind:04X}")));
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                checked_registration_id(0, RegistrationResult::Number(value))
                    .unwrap_err()
                    .contains("non-finite")
            );
        }
    }

    #[test]
    fn compacts_argument_separators_and_checks_utf16_limit() {
        assert_eq!(
            super::argument_text("name, quotes, curves").unwrap(),
            "name,quotes,curves"
        );
        assert!(super::argument_text(&"x".repeat(255)).is_ok());
        assert!(super::argument_text(&"x".repeat(256)).is_err());
        assert!(super::argument_text(&"🦀".repeat(128)).is_err());
    }
}
