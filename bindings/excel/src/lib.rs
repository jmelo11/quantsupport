//! Native Excel XLL bindings for QuantSupport.
//!
//! The actual XLL exports only compile on Windows. The object registry remains
//! portable so its behavior can be unit-tested on any development host.

mod documentation;
mod runtime;

#[cfg(windows)]
mod functions;

#[cfg(windows)]
use xll_rs::{
    convert::return_xl_error,
    returning::XlReturn,
    types::{XLERR_VALUE, XLOPER12, XLTYPE_INT, XLTYPE_NUM},
};

/// Registers every worksheet function when Excel loads the XLL.
#[cfg(windows)]
#[no_mangle]
pub extern "system" fn xlAutoOpen() -> i32 {
    match runtime::registration::windows::register_functions() {
        Ok(()) => match runtime::lifecycle::windows::start() {
            Ok(()) => {
                runtime::browser::windows::start();
                1
            }
            Err(error) => {
                runtime::registration::windows::unregister_functions();
                runtime::registration::windows::show_error(&error);
                0
            }
        },
        Err(error) => {
            runtime::registration::windows::show_error(&error);
            0
        }
    }
}

#[cfg(windows)]
fn worksheet_exports() -> Vec<xll_rs::registry::XllExport> {
    let mut functions: Vec<_> = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
        .into_iter()
        .copied()
        .collect();
    functions.sort_by(|left, right| left.name.cmp(right.name));

    functions
}

/// Releases all in-process object handles when Excel unloads the XLL.
#[cfg(windows)]
#[no_mangle]
pub extern "system" fn xlAutoClose() -> i32 {
    runtime::browser::windows::stop();
    runtime::registration::windows::unregister_functions();
    runtime::lifecycle::windows::stop();
    1
}

/// Supplies the display name shown in Excel's Add-in Manager.
///
/// # Safety
///
/// Excel must pass a valid callback-owned `XLOPER12` pointer.
#[cfg(windows)]
#[no_mangle]
pub unsafe extern "system" fn xlAddInManagerInfo12(action: *const XLOPER12) -> *mut XLOPER12 {
    if !action.is_null() {
        let oper = unsafe { &*action };
        let requests_name = match oper.base_type() {
            XLTYPE_NUM => (unsafe { oper.val.num }) == 1.0,
            XLTYPE_INT => (unsafe { oper.val.w }) == 1,
            _ => false,
        };
        if requests_name {
            return XlReturn::str("QuantSupport").into_raw();
        }
    }
    return_xl_error(XLERR_VALUE)
}

// Excel calls this after copying a result marked with xlbitDLLFree.
#[cfg(windows)]
pub use xll_rs::memory::xlAutoFree12;
