//! Generate clean website posters from the exact downloadable GLB, without UI chrome.
use anyhow::{Context, Result};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let input = args
        .get(1)
        .context("Usage: render_assembly INPUT.glb OUTPUT.png")?;
    let output = args.get(2).context("Missing output PNG")?;
    let mesh = jenga_core::mesh::Mesh::read(Path::new(input))?;
    jenga_tools::assembly_view::raster_lit(&mesh, 1600, 1000, 0.6, 0.9, 1.1, 1.05, 0.5)?
        .save(output)?;
    println!("Rendered {} triangles to {output}", mesh.triangles.len());
    Ok(())
}
