use quantsupport::math::interpolation::{
    extrapolation::ExtrapolationPolicy, interpolator::Interpolator,
};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XLLError, XllError, XLOPER12, XLTYPE_MISSING, XLTYPE_NIL, XLTYPE_NUM};

use crate::{
    functions::helpers::{bool_value, numeric_values, parse_interpolator},
    runtime::interpolation,
};

#[xll_bindgen(
    name = "QS.INTERPOLATE",
    threadsafe,
    category = "QuantSupport - Math",
    help = "Interpolates numeric ranges using Linear, LogLinear, or a natural CubicSpline"
)]
pub fn qs_interpolate(
    point: f64,
    x_values: *const XLOPER12,
    y_values: *const XLOPER12,
    method: Option<String>,
    enable_extrapolation: *const XLOPER12,
) -> Result<f64, XllError> {
    let method = method
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Linear");
    interpolate_1d(
        parse_interpolator(method).map_err(interpolation_value_error)?,
        point,
        x_values,
        y_values,
        enable_extrapolation,
    )
}

#[xll_bindgen(
    name = "QS.INTERPOLATE.LINEAR",
    threadsafe,
    category = "QuantSupport - Math",
    help = "Interpolates numeric ranges linearly with optional linear extrapolation"
)]
pub fn qs_interpolate_linear(
    point: f64,
    x_values: *const XLOPER12,
    y_values: *const XLOPER12,
    enable_extrapolation: *const XLOPER12,
) -> Result<f64, XllError> {
    interpolate_1d(
        Interpolator::Linear,
        point,
        x_values,
        y_values,
        enable_extrapolation,
    )
}

#[xll_bindgen(
    name = "QS.INTERPOLATE.LOG.LINEAR",
    threadsafe,
    category = "QuantSupport - Math",
    help = "Interpolates strictly positive values linearly in log space"
)]
pub fn qs_interpolate_log_linear(
    point: f64,
    x_values: *const XLOPER12,
    y_values: *const XLOPER12,
    enable_extrapolation: *const XLOPER12,
) -> Result<f64, XllError> {
    interpolate_1d(
        Interpolator::LogLinear,
        point,
        x_values,
        y_values,
        enable_extrapolation,
    )
}

#[xll_bindgen(
    name = "QS.INTERPOLATE.CUBIC.SPLINE",
    threadsafe,
    category = "QuantSupport - Math",
    help = "Interpolates a natural cubic spline with zero endpoint second derivatives"
)]
pub fn qs_interpolate_cubic_spline(
    point: f64,
    x_values: *const XLOPER12,
    y_values: *const XLOPER12,
    enable_extrapolation: *const XLOPER12,
) -> Result<f64, XllError> {
    interpolate_1d(
        Interpolator::CubicSpline,
        point,
        x_values,
        y_values,
        enable_extrapolation,
    )
}

#[xll_bindgen(
    name = "QS.INTERPOLATE.BILINEAR",
    threadsafe,
    category = "QuantSupport - Math",
    help = "Interpolates aligned x/y/value ranges over rectangular or sparse slices"
)]
pub fn qs_interpolate_bilinear(
    point_x: f64,
    point_y: f64,
    x_coordinates: *const XLOPER12,
    y_coordinates: *const XLOPER12,
    values: *const XLOPER12,
    extrapolation: Option<String>,
) -> Result<f64, XllError> {
    interpolation::bilinear(
        point_x,
        point_y,
        &numeric_values(x_coordinates, "x_coordinates").map_err(interpolation_value_error)?,
        &numeric_values(y_coordinates, "y_coordinates").map_err(interpolation_value_error)?,
        &numeric_values(values, "values").map_err(interpolation_value_error)?,
        extrapolation_policy(extrapolation.as_deref())?,
    )
    .map_err(interpolation_value_error)
}

#[xll_bindgen(
    name = "QS.INTERPOLATE.TRILINEAR",
    threadsafe,
    category = "QuantSupport - Math",
    help = "Interpolates aligned x/y/z/value ranges over rectangular or sparse slices"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_interpolate_trilinear(
    point_x: f64,
    point_y: f64,
    point_z: f64,
    x_coordinates: *const XLOPER12,
    y_coordinates: *const XLOPER12,
    z_coordinates: *const XLOPER12,
    values: *const XLOPER12,
    extrapolation: Option<String>,
) -> Result<f64, XllError> {
    interpolation::trilinear(
        point_x,
        point_y,
        point_z,
        &numeric_values(x_coordinates, "x_coordinates").map_err(interpolation_value_error)?,
        &numeric_values(y_coordinates, "y_coordinates").map_err(interpolation_value_error)?,
        &numeric_values(z_coordinates, "z_coordinates").map_err(interpolation_value_error)?,
        &numeric_values(values, "values").map_err(interpolation_value_error)?,
        extrapolation_policy(extrapolation.as_deref())?,
    )
    .map_err(interpolation_value_error)
}

fn interpolate_1d(
    method: Interpolator,
    point: f64,
    x_values: *const XLOPER12,
    y_values: *const XLOPER12,
    enable_extrapolation: *const XLOPER12,
) -> Result<f64, XllError> {
    if enable_extrapolation.is_null() {
        return Err(interpolation_value_error("null extrapolation flag"));
    }
    let flag = unsafe { &*enable_extrapolation };
    if flag.base_type() == XLTYPE_NUM && !unsafe { flag.val.num }.is_finite() {
        return Err(interpolation_value_error("nonfinite extrapolation flag"));
    }
    let enable_extrapolation = if matches!(flag.base_type(), XLTYPE_MISSING | XLTYPE_NIL) {
        false
    } else {
        bool_value(enable_extrapolation, "enable_extrapolation")
            .map_err(interpolation_value_error)?
    };
    interpolation::interpolate(
        method,
        point,
        &numeric_values(x_values, "x_values").map_err(interpolation_value_error)?,
        &numeric_values(y_values, "y_values").map_err(interpolation_value_error)?,
        enable_extrapolation,
    )
    .map_err(interpolation_value_error)
}

fn extrapolation_policy(value: Option<&str>) -> Result<ExtrapolationPolicy, XllError> {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "" | "reject" => Ok(ExtrapolationPolicy::Reject),
        "flat" => Ok(ExtrapolationPolicy::Flat),
        _ => Err(interpolation_value_error(
            "use Reject or Flat extrapolation",
        )),
    }
}

fn interpolation_value_error<Error>(_: Error) -> XllError {
    XllError::new(XLLError::VALUE)
}

#[cfg(test)]
mod tests {
    use std::mem::ManuallyDrop;

    use xll_rs::types::{XLOPER12Array, XLOPER12Val, XLTYPE_BOOL, XLTYPE_MULTI};

    use super::*;

    fn range(cells: &mut [XLOPER12], rows: i32, columns: i32) -> XLOPER12 {
        assert_eq!(cells.len(), (rows * columns) as usize);
        XLOPER12 {
            val: XLOPER12Val {
                array: ManuallyDrop::new(XLOPER12Array {
                    lparray: cells.as_mut_ptr(),
                    rows,
                    columns,
                }),
            },
            xltype: XLTYPE_MULTI,
        }
    }

    #[test]
    fn worksheet_methods_accept_ranges_and_preserve_defaults() {
        let mut coordinate_cells = [0.0, 1.0].map(XLOPER12::from_f64);
        let mut value_cells = [1.0, 4.0].map(XLOPER12::from_f64);
        let coordinates = range(&mut coordinate_cells, 2, 1);
        let values = range(&mut value_cells, 1, 2);
        let blank = XLOPER12::nil();
        let enabled = XLOPER12::from_f64(1.0);
        for method in [None, Some(String::new()), Some(" Linear ".into())] {
            assert_eq!(
                qs_interpolate(0.5, &coordinates, &values, method, &blank).unwrap(),
                2.5
            );
        }
        assert_eq!(
            qs_interpolate(0.5, &coordinates, &values, Some("LogLinear".into()), &blank).unwrap(),
            2.0
        );
        assert_eq!(
            qs_interpolate_linear(0.5, &coordinates, &values, &blank).unwrap(),
            2.5
        );
        assert_eq!(
            qs_interpolate_log_linear(0.5, &coordinates, &values, &blank).unwrap(),
            2.0
        );
        assert_eq!(
            qs_interpolate_cubic_spline(0.5, &coordinates, &values, &blank).unwrap(),
            2.5
        );
        assert!(qs_interpolate_linear(2.0, &coordinates, &values, &blank).is_err());
        assert_eq!(
            qs_interpolate_linear(2.0, &coordinates, &values, &enabled).unwrap(),
            7.0
        );
        let mut missing = XLOPER12::nil();
        missing.xltype = XLTYPE_MISSING;
        assert_eq!(
            qs_interpolate_linear(0.5, &coordinates, &values, &missing).unwrap(),
            2.5
        );
    }

    #[test]
    fn worksheet_invalid_inputs_return_error_cells_not_messages() {
        let mut coordinate_cells = [0.0, 1.0].map(XLOPER12::from_f64);
        let mut value_cells = [1.0, 4.0].map(XLOPER12::from_f64);
        let coordinates = range(&mut coordinate_cells, 2, 1);
        let values = range(&mut value_cells, 2, 1);
        let blank = XLOPER12::nil();
        for invalid in [
            XLOPER12::nil(),
            XLOPER12::from_str("1"),
            XLOPER12::from_f64(f64::NAN),
            XLOPER12 {
                val: XLOPER12Val { xbool: 1 },
                xltype: XLTYPE_BOOL,
            },
        ] {
            let error = qs_interpolate_linear(0.5, &coordinates, &invalid, &blank).unwrap_err();
            assert_eq!(error, XllError::new(XLLError::VALUE));
        }
        for flag in [
            XLOPER12::from_str("TRUE"),
            XLOPER12::from_f64(f64::INFINITY),
        ] {
            let error = qs_interpolate_linear(0.5, &coordinates, &values, &flag).unwrap_err();
            assert_eq!(error, XllError::new(XLLError::VALUE));
        }
        assert_eq!(
            qs_interpolate(0.5, &coordinates, &values, Some("unknown".into()), &blank).unwrap_err(),
            XllError::new(XLLError::VALUE)
        );
        assert_eq!(
            qs_interpolate_linear(0.5, std::ptr::null(), &values, &blank).unwrap_err(),
            XllError::new(XLLError::VALUE)
        );
        assert_eq!(
            qs_interpolate_linear(0.5, &coordinates, &values, std::ptr::null()).unwrap_err(),
            XllError::new(XLLError::VALUE)
        );
    }

    #[test]
    fn worksheet_multilinear_ranges_and_policies_match_native_operations() {
        let mut coordinate_x = [0.0, 1.0, 0.0, 1.0].map(XLOPER12::from_f64);
        let mut coordinate_y = [0.0, 0.0, 1.0, 1.0].map(XLOPER12::from_f64);
        let mut coordinate_z = [0.0; 4].map(XLOPER12::from_f64);
        let mut value_cells = [1.0, 3.0, 4.0, 10.0].map(XLOPER12::from_f64);
        let coordinates_x = range(&mut coordinate_x, 2, 2);
        let coordinates_y = range(&mut coordinate_y, 2, 2);
        let coordinates_z = range(&mut coordinate_z, 2, 2);
        let values = range(&mut value_cells, 2, 2);
        assert_eq!(
            qs_interpolate_bilinear(0.5, 0.5, &coordinates_x, &coordinates_y, &values, None)
                .unwrap(),
            4.5
        );
        assert_eq!(
            qs_interpolate_trilinear(
                0.5,
                0.5,
                0.0,
                &coordinates_x,
                &coordinates_y,
                &coordinates_z,
                &values,
                None
            )
            .unwrap(),
            4.5
        );
        assert!(
            qs_interpolate_bilinear(2.0, 0.5, &coordinates_x, &coordinates_y, &values, None)
                .is_err()
        );
        assert_eq!(
            qs_interpolate_bilinear(
                2.0,
                0.5,
                &coordinates_x,
                &coordinates_y,
                &values,
                Some(" flat ".into())
            )
            .unwrap(),
            6.5
        );
        assert_eq!(
            qs_interpolate_trilinear(
                0.5,
                0.5,
                9.0,
                &coordinates_x,
                &coordinates_y,
                &coordinates_z,
                &values,
                Some("Flat".into())
            )
            .unwrap(),
            4.5
        );
        for policy in [None, Some(""), Some(" Reject ")] {
            assert_eq!(
                extrapolation_policy(policy).unwrap(),
                ExtrapolationPolicy::Reject
            );
        }
        assert_eq!(
            extrapolation_policy(Some("linear")).unwrap_err(),
            XllError::new(XLLError::VALUE)
        );
    }
}
