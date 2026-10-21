//! The Slint-generated types.
//!
//! Keeping `include_modules!` in its own module means the rest of the crate
//! imports `crate::ui::AppWindow` and `crate::ui::AppState` explicitly instead
//! of relying on items at the crate root.

slint::include_modules!();
