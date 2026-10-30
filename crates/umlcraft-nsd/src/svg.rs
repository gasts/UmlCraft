//! Diagrams as SVG images, drawn as Structorizer exports them: black lines and texts on white,
//! element colours as set, 1 px lines on pixel centres.

use std::fmt::Write as _;

use crate::draw::{HATCH, HATCH_TILE, INCLUDES_FRAME, Ink, Primitive, draw};
use crate::layout::Layout;
use crate::metrics::Metrics;
use crate::model::{Color, Diagram};

/// The diagram as a standalone SVG document, one unit per pixel.
pub fn render(diagram: &Diagram, m: &Metrics) -> String {
    render_layout(diagram, &Layout::of(diagram, m), m)
}

/// The same for a diagram laid out already (with the same metrics).
pub fn render_layout(diagram: &Diagram, layout: &Layout, m: &Metrics) -> String {
    let (w, h) = layout.size;
    // Lines lie on pixel centres; the image reaches one pixel beyond the frame's right and
    // bottom lines, as Structorizer's PNG export does.
    let (w, h) = (w + 1, h + 1);
    let mut out = String::new();
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" font-family=\"{}\" font-size=\"12\" xml:space=\"preserve\">",
        m.font_family()
    );
    // Disabled elements: a pixel pattern anchored at the image's origin.
    out.push_str(
        "<defs><pattern id=\"hatch\" patternUnits=\"userSpaceOnUse\" width=\"5\" height=\"10\">",
    );
    for (x, y) in HATCH_TILE {
        let _ = write!(
            out,
            "<rect x=\"{x}\" y=\"{y}\" width=\"1\" height=\"1\" fill=\"#{}\"/>",
            HATCH.to_hex()
        );
    }
    out.push_str("</pattern></defs>\n");
    for p in draw(diagram, layout, m) {
        match p {
            Primitive::Area { points, ink } => {
                let points: Vec<String> =
                    points.iter().map(|p| format!("{},{}", p.x, p.y)).collect();
                let _ = writeln!(
                    out,
                    "<polygon points=\"{}\" fill=\"{}\" shape-rendering=\"crispEdges\"/>",
                    points.join(" "),
                    color(ink)
                );
            }
            Primitive::Line { from, to, ink } => {
                let crisp = from.x == to.x || from.y == to.y;
                let _ = writeln!(
                    out,
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" \
                     stroke-width=\"1\" stroke-linecap=\"square\"{}/>",
                    f64::from(from.x) + 0.5,
                    f64::from(from.y) + 0.5,
                    f64::from(to.x) + 0.5,
                    f64::from(to.y) + 0.5,
                    color(ink),
                    if crisp { " shape-rendering=\"crispEdges\"" } else { "" }
                );
            }
            Primitive::RoundFrame { area: (a, b), radius } => {
                let _ = writeln!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" \
                     fill=\"none\" stroke=\"#000000\" stroke-width=\"1\"/>",
                    f64::from(a.x) + 0.5,
                    f64::from(a.y) + 0.5,
                    b.x - a.x,
                    b.y - a.y,
                    radius
                );
            }
            Primitive::Text { at, text, bold, upwards, small } => {
                let weight = if bold { " font-weight=\"bold\"" } else { "" };
                let weight = if small { " font-size=\"8\"" } else { weight };
                let place = if upwards {
                    format!(" transform=\"translate({} {}) rotate(-90)\"", at.x, at.y)
                } else {
                    format!(" x=\"{}\" y=\"{}\"", at.x, at.y)
                };
                // Java's width, so that a line ends where Structorizer's does whatever font
                // the viewer has.
                let width = if small {
                    m.width(&text, false) * 2 / 3
                } else if bold {
                    m.title_width(std::slice::from_ref(&text))
                } else {
                    m.width(&text, false)
                };
                let _ = writeln!(
                    out,
                    "<text{place}{weight} textLength=\"{width}\" lengthAdjust=\"spacingAndGlyphs\">{}</text>",
                    escape(&text)
                );
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

fn color(ink: Ink) -> String {
    match ink {
        Ink::Line => "#000000".into(),
        Ink::Background => "#ffffff".into(),
        Ink::Head | Ink::Comment => format!("#{}", crate::draw::GREY.to_hex()),
        Ink::Hatch => "url(#hatch)".into(),
        Ink::Includes => format!("#{}", INCLUDES_FRAME.to_hex()),
        Ink::Fill(Color(c)) => format!("#{}", Color(c).to_hex()),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
