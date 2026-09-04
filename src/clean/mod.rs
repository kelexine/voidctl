// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: System hygiene clean subsystem entry point and API exports

pub mod classifier;
pub mod privilege;
pub mod select;
pub mod walker;

pub use classifier::{CleanCategory, CleanItem, CleanTarget};
pub use privilege::is_elevated;
pub use select::{CleanError, interactive_select_and_clean};
pub use walker::{CleanReport, calculate_dir_size, scan_hygiene};
