//! Java's text widths for Astah's diagram font: "Dialog" (Arial on Windows) at 12 and 14 px,
//! where every character advances by a whole number of pixels after hinting. Astah sizes tabs and
//! centers names by these widths, though it draws the texts with exact advances.
//!
//! The tables hold the advances of U+0020 to U+00FF as Java 21 in Astah 12 reports them
//! (`FontMetrics::charWidth`); control characters are 0. A string's width is their sum.

/// Advances of plain text, from U+0020.
const PLAIN: [u8; 224] = [
    3, 3, 4, 7, 7, 11, 8, 2, 4, 4, 5, 7, 3, 4, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 3, 3, 7, 7, 7,
    7, 12, 7, 8, 9, 9, 8, 7, 9, 9, 3, 6, 8, 7, 9, 9, 9, 8, 9, 9, 8, 7, 9, 7, 11, 7, 7, 7, 3, 3, 3,
    5, 7, 4, 7, 7, 6, 7, 7, 3, 7, 7, 3, 3, 6, 3, 11, 7, 7, 7, 7, 4, 7, 3, 7, 5, 9, 5, 5, 5, 4, 3,
    4, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 3, 3, 7, 7, 7, 7, 3, 7, 4, 9, 4, 7, 7, 0, 9, 7, 5, 7, 4, 4, 4, 7, 6, 4, 4, 4, 4, 7,
    10, 10, 10, 7, 7, 7, 7, 7, 7, 7, 12, 9, 8, 8, 8, 8, 3, 3, 3, 3, 9, 9, 9, 9, 9, 9, 9, 7, 9, 9,
    9, 9, 9, 7, 8, 8, 7, 7, 7, 7, 7, 7, 11, 6, 7, 7, 7, 7, 3, 3, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 5, 7, 5,
];

/// Advances of bold text, from U+0020.
const BOLD: [u8; 224] = [
    3, 3, 6, 7, 7, 9, 9, 3, 4, 4, 5, 7, 3, 4, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 3, 3, 7, 7, 7, 7,
    12, 8, 8, 8, 8, 7, 6, 8, 8, 3, 7, 8, 7, 10, 8, 9, 8, 9, 8, 8, 7, 8, 8, 12, 8, 7, 7, 4, 3, 4, 7,
    7, 4, 7, 7, 7, 7, 7, 4, 7, 7, 3, 3, 7, 3, 11, 7, 7, 7, 7, 5, 7, 4, 7, 6, 10, 7, 6, 6, 5, 3, 5,
    7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 3, 3, 7, 7, 7, 7, 3, 7, 4, 9, 4, 7, 7, 0, 9, 7, 5, 7, 4, 4, 4, 7, 7, 4, 4, 4, 4, 7, 10,
    10, 10, 7, 8, 8, 8, 8, 8, 8, 13, 8, 7, 7, 7, 7, 3, 3, 3, 3, 9, 8, 9, 9, 9, 9, 9, 7, 9, 8, 8, 8,
    8, 7, 8, 7, 7, 7, 7, 7, 7, 7, 11, 7, 7, 7, 7, 7, 3, 3, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 6, 7, 6,
];

/// Advances of plain text at 14 px, from U+0020.
const PLAIN_14: [u8; 224] = [
    4, 5, 5, 8, 8, 12, 9, 3, 5, 5, 5, 8, 4, 5, 4, 4, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 4, 4, 8, 8, 8,
    8, 14, 9, 9, 10, 10, 9, 9, 11, 9, 3, 6, 9, 8, 11, 9, 11, 9, 11, 10, 9, 9, 9, 9, 13, 8, 9, 8, 4,
    4, 4, 5, 8, 5, 8, 8, 7, 8, 8, 4, 8, 8, 3, 3, 7, 3, 11, 8, 8, 8, 8, 5, 7, 4, 8, 7, 9, 6, 7, 6,
    5, 3, 5, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 4, 5, 8, 8, 8, 8, 3, 8, 5, 10, 4, 8, 8, 0, 10, 8, 6, 8, 5, 5, 5, 8, 8, 5, 5, 5,
    5, 8, 12, 12, 12, 9, 9, 9, 9, 9, 9, 9, 14, 10, 9, 9, 9, 9, 3, 3, 3, 3, 10, 9, 11, 11, 11, 11,
    11, 8, 11, 9, 9, 9, 9, 9, 9, 9, 8, 8, 8, 8, 8, 8, 12, 7, 8, 8, 8, 8, 3, 3, 3, 3, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 7, 8, 7,
];

/// Advances of bold text at 14 px, from U+0020.
const BOLD_14: [u8; 224] = [
    4, 4, 7, 8, 8, 10, 10, 3, 5, 5, 5, 8, 4, 5, 4, 4, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 4, 4, 8, 8, 8,
    9, 14, 9, 10, 10, 10, 9, 9, 11, 10, 4, 8, 10, 8, 13, 10, 11, 9, 11, 10, 9, 8, 10, 9, 13, 9, 10,
    8, 5, 4, 5, 8, 8, 5, 8, 9, 8, 9, 9, 5, 9, 9, 4, 4, 8, 4, 12, 9, 9, 9, 9, 6, 8, 5, 9, 9, 11, 8,
    7, 7, 5, 3, 5, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 4, 4, 8, 8, 8, 8, 3, 8, 5, 10, 5, 8, 8, 0, 10, 8, 6, 8, 5, 5, 5, 8, 8, 5,
    5, 5, 5, 8, 12, 12, 12, 9, 9, 9, 9, 9, 9, 9, 15, 10, 9, 9, 9, 9, 4, 4, 4, 4, 10, 10, 11, 11,
    11, 11, 11, 8, 11, 10, 10, 10, 10, 10, 9, 8, 8, 8, 8, 8, 8, 8, 13, 8, 9, 9, 9, 9, 4, 4, 4, 4,
    9, 9, 9, 9, 9, 9, 9, 8, 9, 9, 9, 9, 9, 7, 9, 7,
];

/// The width Java gives `text` at `size` (12 and 14 are known), or `None` for other sizes and
/// characters beyond Latin-1.
pub fn java_width(text: &str, size: f32, bold: bool) -> Option<f32> {
    let is = |n: f32| (size - n).abs() <= f32::EPSILON;
    let table = match (is(12.0), is(14.0), bold) {
        (true, _, false) => &PLAIN,
        (true, _, true) => &BOLD,
        (_, true, false) => &PLAIN_14,
        (_, true, true) => &BOLD_14,
        _ => return None,
    };
    text.chars()
        .map(|c| {
            let index = (c as u32).checked_sub(0x20)?;
            table.get(index as usize).filter(|&&w| w > 0).map(|&w| f32::from(w))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::java_width;

    #[test]
    fn widths_as_java_measures_them() {
        assert_eq!(java_width("Sequence Diagram", 12.0, false), Some(107.0));
        assert_eq!(java_width("1.1.EstablishAccess", 12.0, false), Some(112.0));
        assert_eq!(java_width("WWWWW", 12.0, true), Some(60.0));
        // Latin-1 letters are known; ö is as wide as o.
        assert_eq!(java_width("ö", 12.0, false), java_width("o", 12.0, false));
        assert_eq!(java_width("x", 13.0, false), None);
        // At 14 px: a class in CD_Dokus.asta, 198 wide in Astah with 10 on either side.
        assert_eq!(java_width("+ contains(obj : T) : boolean", 14.0, false), Some(178.0));
        assert_eq!(java_width("→", 12.0, false), None);
    }
}
