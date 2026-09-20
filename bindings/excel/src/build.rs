fn main() {
    #[cfg(all(windows, target_arch = "x86_64"))]
    xll_rs::build::emit_xll();
}
