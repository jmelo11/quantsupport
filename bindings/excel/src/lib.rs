#[cfg(all(windows, target_arch = "x86_64"))]
mod xll {
    use xll_rs::{register::Reg, returning::XlReturn, types::XLOPER12};

    #[no_mangle]
    pub extern "system" fn qs_version() -> *mut XLOPER12 {
        XlReturn::str(env!("CARGO_PKG_VERSION")).into_raw()
    }

    #[no_mangle]
    pub extern "system" fn xlAutoOpen() -> i32 {
        let reg = Reg::new();

        reg.add(
            "qs_version",
            "Q$",
            "QS.VERSION",
            "",
            "QuantSupport",
            "Returns the QuantSupport add-in version.",
            &[],
        );

        1
    }

    pub use xll_rs::memory::xlAutoFree12;
}

#[cfg(all(windows, target_arch = "x86_64"))]
pub use xll::*;
