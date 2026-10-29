//! Text widths as Structorizer measures its diagram font, Java's logical font Dialog at 12 px
//! with antialiased text and integer metrics: a line is as wide as the sum of its characters'
//! advances, rounded. The tables come from `tools/nsd-metrics/DialogMetrics.java`; the font
//! behind Dialog differs by platform and so do the widths.

use std::sync::OnceLock;

/// The metrics of one platform's Dialog 12.
#[derive(Debug)]
pub struct Metrics {
    plain: Face,
    bold: Face,
}

#[derive(Debug)]
struct Face {
    ascent: i32,
    height: i32,
    /// Advances of U+0020 to U+00FF.
    latin: Vec<f64>,
    /// Advances of the other characters measured.
    extra: Vec<(char, f64)>,
}

impl Face {
    fn advance(&self, c: char) -> f64 {
        let code = c as u32;
        if (0x20..=0xff).contains(&code) {
            return self.latin[(code - 0x20) as usize];
        }
        if let Some((_, w)) = self.extra.iter().find(|(e, _)| *e == c) {
            return *w;
        }
        // Not measured: a wide glyph for East Asian scripts and emoji, else an average one.
        if code >= 0x1100 { 12.0 } else { self.latin[usize::from(b'x' - 0x20)] }
    }
}

impl Metrics {
    /// The metrics of the platform UmlCraft runs on. Only Linux's table is measured so far;
    /// the others use it until theirs are.
    pub fn for_platform() -> &'static Metrics {
        Metrics::linux()
    }

    /// Dialog 12 as Java draws it on Linux (DejaVu Sans).
    pub fn linux() -> &'static Metrics {
        static LINUX: OnceLock<Metrics> = OnceLock::new();
        LINUX.get_or_init(|| parse(include_str!("metrics/dialog12-linux.txt")))
    }

    /// The width Structorizer gives `text` (one line); tabs and control characters count 0.
    pub fn width(&self, text: &str, bold: bool) -> i32 {
        let face = if bold { &self.bold } else { &self.plain };
        let sum: f64 = text.chars().map(|c| face.advance(c)).sum();
        // Java's Math.round: half up.
        (sum + 0.5).floor() as i32
    }

    /// The width of the diagram's title, which is bold: Structorizer cuts the fraction off
    /// here instead of rounding.
    pub fn title_width(&self, lines: &[String]) -> i32 {
        let width = |l: &String| l.chars().map(|c| self.bold.advance(c)).sum::<f64>() as i32;
        lines.iter().map(width).max().unwrap_or(0)
    }

    /// The widest of `lines`.
    pub fn max_width(&self, lines: &[String], bold: bool) -> i32 {
        lines.iter().map(|l| self.width(l, bold)).max().unwrap_or(0)
    }

    /// The fonts an SVG image names: the one Java draws Dialog with on the platform the image
    /// is made on first (DejaVu Sans on Linux, Arial on Windows, Lucida Grande on macOS).
    pub fn font_family(&self) -> &'static str {
        if cfg!(windows) {
            "Arial, Dialog, 'DejaVu Sans', sans-serif"
        } else if cfg!(target_os = "macos") {
            "'Lucida Grande', Dialog, Arial, sans-serif"
        } else {
            "'DejaVu Sans', Dialog, Arial, sans-serif"
        }
    }

    /// The distance between the lines of an element's text.
    pub fn line(&self) -> i32 {
        self.plain.ascent
    }

    /// The distance between the lines of the diagram's title, which is bold.
    pub fn title_line(&self) -> i32 {
        self.bold.height
    }
}

fn parse(table: &str) -> Metrics {
    let mut faces: [Option<Face>; 2] = [None, None];
    for line in table.lines() {
        let mut words = line.split_whitespace();
        let index = match words.next() {
            Some("plain") => 0,
            Some("bold") => 1,
            _ => continue,
        };
        let rest: Vec<&str> = words.collect();
        let face = faces[index].get_or_insert_with(|| Face {
            ascent: 0,
            height: 0,
            latin: Vec::new(),
            extra: Vec::new(),
        });
        match rest.first() {
            Some(&"ascent") => {
                let number = |key: &str| {
                    let at = rest.iter().position(|w| *w == key).expect("metrics key");
                    rest[at + 1].parse().expect("metrics number")
                };
                face.ascent = number("ascent");
                face.height = number("height");
            }
            Some(&"extra") => {
                for item in &rest[1..] {
                    let (code, w) = item.split_once('=').expect("U+XXXX=width");
                    let c = u32::from_str_radix(&code[2..], 16).ok().and_then(char::from_u32);
                    face.extra.push((c.expect("code point"), w.parse().expect("width")));
                }
            }
            _ => face.latin = rest.iter().map(|w| w.parse().expect("width")).collect(),
        }
    }
    let [Some(plain), Some(bold)] = faces else { panic!("metrics table lacks a face") };
    assert_eq!(plain.latin.len(), 224, "plain advances of U+0020..U+00FF");
    assert_eq!(bold.latin.len(), 224, "bold advances of U+0020..U+00FF");
    Metrics { plain, bold }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Widths Structorizer's instructions have, less the padding of 20, measured in its PNG
    /// exports without highlighting (`breiten` series of the reference folder).
    #[test]
    fn linux_widths_match_structorizer() {
        let measured = [
            ("x", 6),
            ("abc", 21),
            ("hello", 28),
            ("hello world", 64),
            ("WWWWW", 55),
            ("iiiii", 15),
            ("12345", 35),
            ("a+b", 22),
            ("a <- 1", 31),
            ("ab <- c + d", 59),
            ("x <- x + 1", 49),
            ("\"text\"", 31),
            ("f(x)", 17),
            ("a.b.c", 27),
            ("OUTPUT x", 60),
            ("a != b", 31),
            ("a <= b", 35),
            ("a and b", 43),
            ("mmmmmmmmmm", 110),
            ("0123456789", 70),
            ("a <- b <- c", 55),
        ];
        let m = Metrics::linux();
        for (text, width) in measured {
            assert_eq!(m.width(text, false), width, "{text}");
        }
    }

    #[test]
    fn bold_sums_are_rounded_half_up() {
        let m = Metrics::linux();
        assert_eq!(m.width("leer", true), 24);
        assert_eq!(m.width("anweisung", true), 65);
        assert_eq!(m.width("mmmm", true), 46);
    }

    #[test]
    fn line_distances() {
        let m = Metrics::linux();
        assert_eq!(m.line(), 13);
        assert_eq!(m.title_line(), 17);
        assert_eq!(m.width("∅", false), 10);
    }
}
