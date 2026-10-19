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

#[cfg(windows)]
pub mod windows {
    use std::cell::RefCell;
    use xll_rs::{
        entrypoint::{excel12, excel_free, XLRET_SUCCESS},
        memory::xlAutoFree12,
        types::{XLBIT_XL_FREE, XLF_REGISTER, XLOPER12, XLTYPE_ERR, XLTYPE_NUM, XL_GET_NAME},
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
        if status != XLRET_SUCCESS {
            return Err(format!("Excel callback failed with code {status}"));
        }
        if result.base_type() == XLTYPE_NUM {
            let id = unsafe { result.val.num };
            if id.is_finite() && id > 0.0 {
                return Ok(id);
            }
        }
        if result.base_type() == XLTYPE_ERR {
            return Err(format!(
                "Excel rejected registration (worksheet error {})",
                unsafe { result.val.err }
            ));
        }
        Err("Excel did not return a valid numeric registration ID".into())
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
                XLOPER12::from_f64(0.0),
                XLOPER12::from_f64(f64::NAN),
                XLOPER12::from_bool(true),
            ] {
                assert!(registration_id(XLRET_SUCCESS, &result).is_err());
            }
            assert_eq!(
                registration_id(XLRET_SUCCESS, &XLOPER12::from_f64(42.0)).unwrap(),
                42.0
            );
        }
    }
}

#[cfg(test)]
mod tests {
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
