//! The exact same semantic gallery as ui_gallery: only the skin choice changes.
//! --skin=vapor95|black-ice|swiss-signal --mono|--ansi16|--ansi256|--truecolor
//! --dump [--width=80 --height=24] captures the settled screen without a terminal.
//! Use --dump-ansi to retain colors for an external screenshot renderer.
#[path = "ui_support/gallery.rs"]
mod gallery;
#[path = "ui_support/options.rs"]
mod options;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    gallery::run()
}
