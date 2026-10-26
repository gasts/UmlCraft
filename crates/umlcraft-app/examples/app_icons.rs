//! Renders the app icon into the Windows `.ico` and the macOS `.icns`.
//!
//! Run `cargo run -p umlcraft-app --example app_icons` after changing one of the drawings. The
//! large drawing is also the Linux icon; small sizes come from their own drawings in
//! `packaging/icons`, which keep the shapes on whole pixels.

use std::path::Path;

use resvg::{tiny_skia, usvg};

const LARGE: &str = "packaging/linux/com.umlcraft.app.svg";
const SMALL_16: &str = "packaging/icons/umlcraft-16.svg";
const SMALL_32: &str = "packaging/icons/umlcraft-32.svg";

const ICO: &str = "packaging/windows/umlcraft.ico";
const ICNS: &str = "packaging/macos/UmlCraft.icns";

/// Sizes Windows picks from for the taskbar, title bar, Explorer and the Start menu.
const ICO_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

/// The `.icns` entries as PNG, by type code and size in pixels.
const ICNS_ENTRIES: [(&[u8; 4], u32); 10] = [
    (b"icp4", 16),
    (b"ic11", 32),
    (b"icp5", 32),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic13", 256),
    (b"ic08", 256),
    (b"ic14", 512),
    (b"ic09", 512),
    (b"ic10", 1024),
];

struct Drawings {
    large: usvg::Tree,
    small_16: usvg::Tree,
    small_32: usvg::Tree,
}

impl Drawings {
    /// The drawing made for a tile of `size` pixels.
    fn for_size(&self, size: f32) -> &usvg::Tree {
        if size <= 24.0 {
            &self.small_16
        } else if size <= 40.0 {
            &self.small_32
        } else {
            &self.large
        }
    }
}

fn main() -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let load = |path: &str| -> anyhow::Result<usvg::Tree> {
        let svg = std::fs::read_to_string(root.join(path))?;
        Ok(usvg::Tree::from_str(&svg, &usvg::Options::default())?)
    };
    let drawings =
        Drawings { large: load(LARGE)?, small_16: load(SMALL_16)?, small_32: load(SMALL_32)? };

    let mut images = Vec::new();
    for size in ICO_SIZES {
        images.push((size, render(&drawings, size, 0.0)?));
    }
    std::fs::create_dir_all(root.join(ICO).parent().unwrap())?;
    std::fs::write(root.join(ICO), ico(&images))?;

    // macOS draws app icons on a grid of 1024 with the tile at 824 in the middle.
    let mut entries = Vec::new();
    for (code, size) in ICNS_ENTRIES {
        let inset = size as f32 * 100.0 / 1024.0;
        entries.push((code, render(&drawings, size, inset)?));
    }
    std::fs::write(root.join(ICNS), icns(&entries))?;
    Ok(())
}

/// A PNG of `size` pixels with the tile `inset` pixels from each edge.
fn render(drawings: &Drawings, size: u32, inset: f32) -> anyhow::Result<Vec<u8>> {
    let tile = size as f32 - 2.0 * inset;
    let tree = drawings.for_size(tile);
    let scale = tile / tree.size().width();
    let mut pixmap = tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| anyhow::anyhow!("the icon would be empty"))?;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, inset, inset);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(pixmap.encode_png()?)
}

/// An `.ico` file with PNG images, which Windows reads since Vista.
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(0u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend((images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (size, png) in images {
        // 256 is written as 0.
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((png.len() as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        out.extend(png);
    }
    out
}

/// An `.icns` file: a header, then one block per image, lengths big-endian.
fn icns(entries: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (code, png) in entries {
        body.extend(*code);
        body.extend((8 + png.len() as u32).to_be_bytes());
        body.extend(png);
    }
    let mut out = Vec::new();
    out.extend(b"icns");
    out.extend((8 + body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}
