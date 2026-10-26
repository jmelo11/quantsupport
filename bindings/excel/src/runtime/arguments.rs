//! Typed worksheet arguments decoded inside the caller's ownership scope.

use xll_rs::types::{
    XllError, XLERR_VALUE, XLOPER12, XLTYPE_ERR, XLTYPE_INT, XLTYPE_MISSING, XLTYPE_NIL, XLTYPE_NUM,
};

pub trait Argument: Sized {
    fn read(value: &XLOPER12) -> Result<Self, XllError>;
}

pub fn decode<T: Argument>(value: *const XLOPER12) -> Result<T, XllError> {
    if value.is_null() {
        return Err(XllError::from(XLERR_VALUE));
    }
    let value = unsafe { &*value };
    if value.base_type() == XLTYPE_ERR {
        return Err(XllError::from(unsafe { value.val.err }));
    }
    T::read(value)
}

impl Argument for String {
    fn read(value: &XLOPER12) -> Result<Self, XllError> {
        value.as_string().ok_or_else(|| XllError::from(XLERR_VALUE))
    }
}
impl Argument for bool {
    fn read(value: &XLOPER12) -> Result<Self, XllError> {
        value.as_bool().ok_or_else(|| XllError::from(XLERR_VALUE))
    }
}
impl Argument for f64 {
    fn read(value: &XLOPER12) -> Result<Self, XllError> {
        match value.base_type() {
            XLTYPE_NUM => Ok(unsafe { value.val.num }),
            XLTYPE_INT => Ok(f64::from(unsafe { value.val.w })),
            _ => Err(XllError::from(XLERR_VALUE)),
        }
    }
}
impl Argument for i32 {
    fn read(value: &XLOPER12) -> Result<Self, XllError> {
        match value.base_type() {
            XLTYPE_INT => Ok(unsafe { value.val.w }),
            XLTYPE_NUM => Ok(unsafe { value.val.num } as i32),
            _ => Err(XllError::from(XLERR_VALUE)),
        }
    }
}
impl<T: Argument> Argument for Option<T> {
    fn read(value: &XLOPER12) -> Result<Self, XllError> {
        if matches!(value.base_type(), XLTYPE_MISSING | XLTYPE_NIL) {
            Ok(None)
        } else {
            T::read(value).map(Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_and_required_scalars_preserve_excel_errors_and_types() {
        assert_eq!(decode::<f64>(&XLOPER12::from_int(3)), Ok(3.0));
        assert_eq!(decode::<i32>(&XLOPER12::from_f64(3.5)), Ok(3));
        assert_eq!(decode::<Option<f64>>(&XLOPER12::nil()), Ok(None));
        assert_eq!(decode::<bool>(&XLOPER12::from_bool(true)), Ok(true));
        assert!(decode::<f64>(&XLOPER12::nil()).is_err());
        assert!(decode::<String>(&XLOPER12::from_f64(1.0)).is_err());
        assert!(decode::<f64>(std::ptr::null()).is_err());
    }
}
