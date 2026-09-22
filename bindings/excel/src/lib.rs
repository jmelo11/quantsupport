//! Native Excel XLL bindings for QuantSupport.
//!
//! The actual XLL exports only compile on Windows. The object registry remains
//! portable so its behavior can be unit-tested on any development host.

#[cfg_attr(not(windows), allow(dead_code))]
mod registry;

#[cfg(windows)]
mod functions;

#[cfg(windows)]
use registry::with_registry_mut;
#[cfg(windows)]
use xll_rs::{
    convert::return_xl_error,
    register::Reg,
    returning::XlReturn,
    types::{XLERR_VALUE, XLOPER12, XLTYPE_INT, XLTYPE_NUM},
};

/// Registers every worksheet function when Excel loads the XLL.
#[cfg(windows)]
#[no_mangle]
pub extern "system" fn xlAutoOpen() -> i32 {
    let registry = Reg::new();
    let mut functions: Vec<_> = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
        .into_iter()
        .copied()
        .collect();
    functions.sort_by(|left, right| left.name.cmp(right.name));

    for function in functions {
        for excel_name in std::iter::once(function.name).chain(function.aliases.iter().copied()) {
            if registry
                .add(
                    function.rust_name,
                    function.type_str,
                    excel_name,
                    function.arg_names,
                    function.category,
                    function.help,
                    function.arg_help,
                )
                .is_err()
            {
                return 0;
            }
        }
    }
    1
}

/// Releases all in-process object handles when Excel unloads the XLL.
#[cfg(windows)]
#[no_mangle]
pub extern "system" fn xlAutoClose() -> i32 {
    with_registry_mut(|registry| registry.clear());
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
