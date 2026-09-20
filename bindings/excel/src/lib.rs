pub mod registry;

use crate::registry::*;

use xll_rs::convert::return_xl_error;
use xll_rs::register::Reg;
use xll_rs::returning::XlReturn;
use xll_rs::types::*;


#[no_mangle]
pub extern "system" fn xl_add(a: *const XLOPER12, b: *const XLOPER12) -> *mut XLOPER12 {
    if a.is_null() || b.is_null() {
        return return_xl_error(XLERR_VALUE);
    }
    unsafe {
        let av = (*a).as_f64().unwrap_or(0.0);
        let bv = (*b).as_f64().unwrap_or(0.0);
        XlReturn::num(av + bv).into_raw()
    }
}

#[no_mangle]
pub extern "system" fn xlAutoOpen() -> i32 {
    let reg = Reg::new();

    let _  = reg.add(
        "xl_add",
        "QQQ$",
        "ADD.RUST",
        "a, b",
        "xll-rs",
        "Adds two numbers",
        &["First number", "Second number"],
    );

    let _  = reg.add(
        "create_fx_store",
        "Q",
        "QS.CREATE_FX_STORE",
        "",
        "QS",
        "Creates a new FX store and returns its identifier",
        &[],
    );

    let _ = reg.add(
        "add_fx_quote",
        "QQQQQ",
        "QS.ADD.FX.QUOTE",
        "target,ccy1,ccy2,quote",
        "QuantSupport",
        "Adds an FX quote to an FxStore",
        &[
            "FxStore handle",
            "Base currency",
            "Quote currency",
            "FX rate",
        ],
    );

    let _  = reg.add(
        "get_fx_quote",
        "QQQ",
        "QS.GET_FX_QUOTE",
        "target, ccy1, ccy2",
        "QS",
        "Retrieves the FX quote between two currencies from the specified FX store",
        &["FX Store Identifier", "Base Currency", "Quote Currency"],
    );

    1
}

#[no_mangle]
pub extern "system" fn xlAutoClose() -> i32 {
    1
}

#[no_mangle]
pub extern "system" fn xlAddInManagerInfo12(action: *const XLOPER12) -> *mut XLOPER12 {
    if !action.is_null() {
        let oper = unsafe { &*action };
        let is_one = match oper.base_type() {
            XLTYPE_NUM => (unsafe { oper.val.num }) == 1.0,
            XLTYPE_INT => (unsafe { oper.val.w }) == 1,
            _ => false,
        };
        if is_one {
            return XlReturn::str("xll-rs example").into_raw();
        }
    }
    return_xl_error(XLERR_VALUE)
}

// Excel calls this after it copies results with xlbitDLLFree set
pub use xll_rs::memory::xlAutoFree12;