use crate::utils::errors::Result;

/// Trait for types that can produce a visual plot saved to a file.
pub trait Plot {
    /// Renders a plot and writes it to the file at `path`.
    ///
    /// # Errors
    /// Returns an error if the plot cannot be rendered or the file cannot be written.
    fn plot(&self, path: &str) -> Result<()>;
}

#[cfg(feature = "plot")]
pub(crate) mod plotting {
    pub use plotters::prelude::*;

    /// Palette used for the three exposure series.
    pub const EPE_COLOR: RGBColor = RGBColor(31, 119, 180); // steel blue
    pub const ENE_COLOR: RGBColor = RGBColor(44, 160, 44); // green
    pub const EE_COLOR: RGBColor = RGBColor(214, 39, 40); // red

    pub const GRID_COLOR: RGBColor = RGBColor(220, 220, 220);
    pub const BG_COLOR: RGBColor = RGBColor(255, 255, 255);
    pub const ZERO_LINE_COLOR: RGBColor = RGBColor(160, 160, 160);
}
