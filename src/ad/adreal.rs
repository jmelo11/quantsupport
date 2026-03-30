//! Re-export hub — all public items from the split AD submodules are
//! re-exported here so that every existing `use crate::ad::adreal::Foo`
//! path continues to work without modification.

// Re-export everything from the submodules.
pub use super::scalar::*;
pub use super::forward::*;
pub use super::constant::*;
pub use super::expr::*;
pub use super::dual::*;

// ═══════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ad::tape::Tape;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn with_tape_test<F: FnOnce()>(f: F) {
        let _g = TEST_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        f();
        Tape::stop_recording_fwd();
    }

    const EPS: f64 = 1e-10;
    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    #[test]
    fn compare_and_flatten() {
        with_tape_test(|| {
            let x = DualFwd::new(5.0);
            let y = abs(x - 2.0);
            assert!(y > 2.0);
            let z: DualFwd = (y + 1.0).into();
            assert_eq!(z.value(), 4.0);
        });
    }

    #[test]
    fn backprop_basic() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let a = DualFwd::new(3.0);
            let b = DualFwd::new(4.0);
            let out: DualFwd = (a * b).sin().into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap().value(), 1.0);
        });
    }

    #[test]
    fn test_late_tape_recording() {
        with_tape_test(|| {
            let mut a = DualFwd::new(3.0);
            Tape::start_recording_fwd();
            a.put_on_tape();
            let expr = a * a;
            let out: DualFwd = expr.into();
            out.backward().unwrap();
            assert_eq!(a.adjoint().unwrap().value(), 6.0);
        });
    }

    #[test]
    fn backprop_with_const() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let a = DualFwd::new(3.0);
            let out: DualFwd = (a * 4.0).sin().into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap().value(), 1.0);
        });
    }

    #[test]
    fn tape_reset() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let a = DualFwd::new(3.0);
            let b = DualFwd::new(4.0);
            let out: DualFwd = (a * b).sin().into();
            out.backward().unwrap();
            assert_eq!(out.adjoint().unwrap().value(), 1.0);
            Tape::reset_adjoints_fwd();
            assert_eq!(out.adjoint().unwrap().value(), 0.0);
        });
    }

    #[test]
    fn check_exp_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let out: DualFwd = exp(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), f64::exp(2.0)));
        });
    }

    #[test]
    fn check_log_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let out: DualFwd = log(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 0.5));
        });
    }

    #[test]
    fn check_sqrt_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(4.0);
            let out: DualFwd = sqrt(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 0.25));
        });
    }

    #[test]
    fn check_sin_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(0.0);
            let out: DualFwd = sin(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 1.0));
        });
    }

    #[test]
    fn check_cos_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(0.0);
            let out: DualFwd = cos(x).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 0.0));
        });
    }

    #[test]
    fn check_pow_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let out: DualFwd = x.pow_expr(Const::<ADForward>::scalar(3.0)).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 12.0)); // 3x^2 at x=2
        });
    }

    #[test]
    fn check_add_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let y = DualFwd::new(3.0);
            let out: DualFwd = (x + y).into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap().value(), 1.0);
            assert_eq!(y.adjoint().unwrap().value(), 1.0);
        });
    }

    #[test]
    fn check_mul_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(4.0);
            let y = DualFwd::new(2.0);
            let out: DualFwd = (x * y).into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap().value(), 2.0);
            assert_eq!(y.adjoint().unwrap().value(), 4.0);
        });
    }

    #[test]
    fn check_div_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(6.0);
            let y = DualFwd::new(3.0);
            let out: DualFwd = (x / y).into();
            out.backward().unwrap();
            assert!(approx(x.adjoint().unwrap().value(), 1.0 / 3.0));
            assert!(approx(y.adjoint().unwrap().value(), -6.0 / 9.0));
        });
    }

    #[test]
    fn check_max_derivative() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let x = DualFwd::new(2.0);
            let y = DualFwd::new(3.0);
            let out: DualFwd = max(x, y).into();
            out.backward().unwrap();
            assert_eq!(x.adjoint().unwrap().value(), 0.0);
            assert_eq!(y.adjoint().unwrap().value(), 1.0);
        });
    }

    #[test]
    fn test_reassigning() {
        with_tape_test(|| {
            Tape::start_recording_fwd();
            let a0 = DualFwd::new(5.0);
            let b = DualFwd::new(3.0);
            let mut a = a0;
            a *= b;
            let c = a;
            assert_eq!(c.value(), 15.0);
            c.backward().unwrap();
            assert_eq!(a0.adjoint().unwrap().value(), 3.0);
            assert_eq!(b.adjoint().unwrap().value(), 5.0);
        });
    }

    #[test]
    fn multithread_recording() {
        with_tape_test(|| {
            let handle = std::thread::spawn(|| {
                Tape::start_recording_fwd();
                let x = DualFwd::new(2.0);
                let y = DualFwd::new(3.0);
                let out: DualFwd = (x * y + x).into();
                out.backward().unwrap();
                (
                    x.adjoint().unwrap().value(),
                    y.adjoint().unwrap().value(),
                    out.adjoint().unwrap().value(),
                )
            });
            let (dx, dy, dout) = handle.join().unwrap();
            assert_eq!(dx, 4.0);
            assert_eq!(dy, 2.0);
            assert_eq!(dout, 1.0);
        });
    }

    // == Forward mode ====================================================

    #[test]
    fn forward_x_squared() {
        let x = ADForward::var(3.0);
        let y = x * x;
        assert!(approx(y.val, 9.0));
        assert!(approx(y.dot, 6.0));
        assert!(approx(y.dot2, 2.0));
    }

    #[test]
    fn forward_x_cubed() {
        let x = ADForward::var(2.0);
        let y = x * x * x;
        assert!(approx(y.val, 8.0));
        assert!(approx(y.dot, 12.0));
        assert!(approx(y.dot2, 12.0));
    }

    #[test]
    fn forward_exp() {
        let x = ADForward::var(1.0);
        let y = x.exp();
        let e = 1.0_f64.exp();
        assert!(approx(y.val, e));
        assert!(approx(y.dot, e));
        assert!(approx(y.dot2, e));
    }

    #[test]
    fn forward_ln() {
        let x = ADForward::var(2.0);
        let y = x.ln();
        assert!(approx(y.val, 2.0_f64.ln()));
        assert!(approx(y.dot, 0.5));
        assert!(approx(y.dot2, -0.25));
    }

    #[test]
    fn forward_sin() {
        let x = ADForward::var(1.0);
        let y = x.sin();
        assert!(approx(y.val, 1.0_f64.sin()));
        assert!(approx(y.dot, 1.0_f64.cos()));
        assert!(approx(y.dot2, -1.0_f64.sin()));
    }

    // == Mixed mode (Dual<ADForward>) =====================================

    #[test]
    fn mixed_x_squared() {
        Tape::start_recording_fwd();
        let x_inner = ADForward::var(3.0);
        let x = Dual::<ADForward>::new_from_inner(x_inner);
        let y = x * x;
        let out: Dual<ADForward> = y.into();
        out.backward().unwrap();
        let adj = x.adjoint().unwrap();
        assert!(approx(adj.val, 6.0));
        assert!(approx(adj.dot, 2.0));
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
    }

    #[test]
    fn mixed_exp() {
        Tape::start_recording_fwd();
        let x_inner = ADForward::var(1.0);
        let x = Dual::<ADForward>::new_from_inner(x_inner);
        let y: Dual<ADForward> = FloatExt::exp(x).into();
        y.backward().unwrap();
        let adj = x.adjoint().unwrap();
        let e = 1.0_f64.exp();
        assert!(approx(adj.val, e));
        assert!(approx(adj.dot, e));
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
    }

    // == num_complex =====================================================

    #[test]
    fn complex_ad_forward_basic() {
        use num_complex::Complex;
        let a = Complex::new(ADForward::constant(1.0), ADForward::constant(2.0));
        let b = Complex::new(ADForward::constant(3.0), ADForward::constant(-1.0));
        let c = a * b;
        assert!(approx(c.re.val, 5.0));
        assert!(approx(c.im.val, 5.0));
    }
}
