use std::collections::HashMap;
use std::str::FromStr;
use xll_rs::returning::XlReturn;
use xll_rs::types::*;
use xll_rs::convert::return_xl_error;

use quantsupport::prelude::*;

pub enum QSObject{
    FxStore(FxStore),
}

impl QSObject{
    pub fn as_identifier(&self, pos: u64) -> String{
        format!("QSObject:{}:{}",String::from(self),  pos)
    }
}

impl From<&QSObject> for String{
    fn from(obj: &QSObject) -> Self {
        match obj{
            QSObject::FxStore(_) => "FxStore".to_string(),
        }
    }
}

pub struct Registry{
    objects: HashMap<u64, QSObject>,
}

thread_local! {
    static REGISTRY: std::cell::RefCell<Registry> = std::cell::RefCell::new(Registry{
        objects: HashMap::new(),
    });
}

#[no_mangle]
pub extern "system" fn create_fx_store() -> *mut XLOPER12 {
    let store = QSObject::FxStore(FxStore::new());
    let id = REGISTRY.with(|r| {
        let mut registry = r.borrow_mut();
        let pos = registry.objects.len() as u64 + 1;
        let id = store.as_identifier(pos);
        registry.objects.insert(pos, store);
        id
    });
    XlReturn::str(&id).into_raw()
}

pub fn parse_identifier(id: &str) -> Result<u64> {
    let parts: Vec<&str> = id.split(':').collect();
    if parts.len() != 3 {
        return Err(QSError::InvalidValueErr(id.to_string()));
    }
    if parts[0] != "QSObject" {
        return Err(QSError::InvalidValueErr(id.to_string()));
    }
    let pos = parts[2].parse::<u64>().map_err(|_| QSError::InvalidValueErr(id.to_string()))?;
    Ok(pos)
}

#[no_mangle]
pub extern "system" fn add_fx_quote(
    target: *const XLOPER12,
    ccy1: *const XLOPER12,
    ccy2: *const XLOPER12,
    quote: *const XLOPER12,
) -> *mut XLOPER12 {
    if target.is_null()
        || ccy1.is_null()
        || ccy2.is_null()
        || quote.is_null()
    {
        return return_xl_error(XLERR_VALUE);
    }

    unsafe {
        let target_str = match (*target).as_string() {
            Some(v) => v,
            None => return XlReturn::str("target is not a string").into_raw(),
        };

        let id = match parse_identifier(&target_str) {
            Ok(v) => v,
            Err(e) => {
                return XlReturn::str(&format!("invalid identifier '{target_str}': {e}"))
                .into_raw();
            }
        };

        let ccy1_str = match (*ccy1).as_string() {
            Some(v) => v,
            None => return XlReturn::str("ccy1 is not a string").into_raw(),
        };

        let ccy1 = match Currency::from_str(&ccy1_str) {
            Ok(v) => v,
            Err(e) => {
                return XlReturn::str(&format!(
                    "invalid currency '{ccy1_str}': {e}"
                ))
                .into_raw();
            }
        };

        let ccy2_str = match (*ccy2).as_string() {
            Some(v) => v,
            None => return XlReturn::str("ccy2 is not a string").into_raw(),
        };

        let ccy2 = match Currency::from_str(&ccy2_str) {
            Ok(v) => v,
            Err(e) => {
                return XlReturn::str(&format!(
                    "invalid currency '{ccy2_str}': {e}"
                ))
                .into_raw();
            }
        };

        let quote = match (*quote).as_f64() {
            Some(v) => v,
            None => return XlReturn::str("quote is not numeric").into_raw(),
        };

        let result = REGISTRY.with(|r| {
            let mut registry = r.borrow_mut();

            match registry.objects.get_mut(&id) {
                Some(QSObject::FxStore(store)) => {
                    store.add_fx_rate(
                        ccy1,
                        ccy2,
                        DualFwd::new(quote),
                    );

                    Ok(())
                }

                None => Err(format!(
                    "object {id} does not exist"
                )),
            }
        });

        match result {
            Ok(()) => XlReturn::bool(true).into_raw(),
            Err(e) => XlReturn::str(&e).into_raw(),
        }
    }
}
#[no_mangle]
pub extern "system" fn get_fx_quote(target:  *const XLOPER12, ccy1: *const XLOPER12, ccy2: *const XLOPER12) -> *mut XLOPER12 {
    if target.is_null() || ccy1.is_null()  || ccy2.is_null() {
        return return_xl_error(XLERR_VALUE);
    }
    unsafe {
        let id = (*target).as_string().as_deref().and_then(|s| Some(parse_identifier(s).unwrap()));
        let ccy1 = (*target).as_string().as_deref().and_then(|s| Some(Currency::from_str(s).unwrap()));
        let ccy2 = (*target).as_string().as_deref().and_then(|s| Some(Currency::from_str(s).unwrap()));

        if let (Some(id), Some(ccy1), Some(ccy2)) = (id, ccy1, ccy2) {
            REGISTRY.with(|r| {
                let registry = r.borrow();
                if let Some(QSObject::FxStore(store)) = registry.objects.get(&id) {                    
                    if let Ok(rate) = store.get_fx_rate(ccy1, ccy2) {
                        let val = rate.value();
                        return XlReturn::num(val).into_raw();
                    } else {
                        return return_xl_error(XLERR_NA);
                    }
                } else {
                    return return_xl_error(XLERR_NA);
                }
            })
        } else {
            return return_xl_error(XLERR_VALUE);
        }
    }
}
