//! Trusted M0 RGB fixture -> public LibGibson scene -> headless ANSI capture.
use gibson::context::RenderMode;
use gibson::raster::RgbRaster;
use gibson::{Context, Node};
use std::{env, error::Error, fs, io::Read};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: math_m0_capture INPUT.rgb OUTPUT.ansi half|mono".into());
    }
    let mut data = Vec::new();
    fs::File::open(&args[1])?
        .take(80 * 40 * 3 + 1)
        .read_to_end(&mut data)?;
    if data.len() != 80 * 40 * 3 {
        return Err("fixture must be exactly 9600 bytes".into());
    }
    let raster = RgbRaster::from_rgb_bytes(80, 40, 80 * 3, &data)?;
    let surface = match args[3].to_str() {
        Some("half") => raster.to_surface(),
        Some("mono") => raster.to_mono_surface(),
        _ => return Err("mode must be half or mono".into()),
    };
    let mut ctx = Context::headless(RenderMode::Fullscreen, 80, 20);
    ctx.set_root(Node::raster(surface));
    ctx.render()?;
    let output = ctx.take_rendered_bytes();
    if output.is_empty() {
        return Err("empty scene capture".into());
    }
    fs::write(&args[2], output)?;
    Ok(())
}
