//! Interactive component gallery. Tab/Enter navigate; "Switch skin" changes the
//! complete design grammar. --page=0|1|2 selects the initial page.
//! --dump --width=80 --height=24 --skin=vapor95 --mono is a deterministic capture.
#[path = "ui_support/gallery.rs"]
mod gallery;
#[path = "ui_support/options.rs"]
mod options;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    gallery::run()
}
