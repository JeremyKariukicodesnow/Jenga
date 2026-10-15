//! Rust decoder for KiCad NewStroke's Hershey coordinate data.
//! Font data copyright Vladimir Uryvaev and KiCad contributors; GPL-2.0-or-later,
//! with additional CJK notices preserved in resources/fonts/newstroke_font.cpp.
use crate::scene::Point;
include!(concat!(env!("OUT_DIR"), "/newstroke.rs"));
fn glyph(c: char) -> &'static [u8] {
    let index = (c as usize)
        .checked_sub(32)
        .filter(|i| *i < GLYPHS.len())
        .unwrap_or('?' as usize - 32);
    GLYPHS[index].as_bytes()
}
fn advance(c: char) -> f64 {
    let b = glyph(c);
    if b.len() < 2 {
        return 0.5;
    }
    (b[1] as f64 - b[0] as f64) / 21.
}
pub fn paths(text: &str, size: f64, left: bool) -> Vec<Vec<Point>> {
    let mut paths = vec![];
    let lines: Vec<_> = text.lines().collect();
    let height = (lines.len().saturating_sub(1)) as f64 * size * 1.5;
    for (line_index, line) in lines.iter().enumerate() {
        let width = line.chars().map(advance).sum::<f64>() * size;
        let mut cursor = if left { 0. } else { -width / 2. };
        let baseline = size * 0.45 - height / 2. + line_index as f64 * size * 1.5;
        for c in line.chars() {
            let b = glyph(c);
            let start = b.first().copied().unwrap_or(b'R') as f64 - b'R' as f64;
            let mut stroke = vec![];
            for pair in b.get(2..).unwrap_or_default().as_chunks::<2>().0 {
                if pair == b" R" {
                    if stroke.len() > 1 {
                        paths.push(std::mem::take(&mut stroke));
                    } else {
                        stroke.clear();
                    }
                } else {
                    stroke.push((
                        cursor + (pair[0] as f64 - b'R' as f64 - start) * size / 21.,
                        baseline + (pair[1] as f64 - b'R' as f64 - 8.) * size / 21.,
                    ));
                }
            }
            if stroke.len() > 1 {
                paths.push(stroke);
            }
            cursor += advance(c) * size;
        }
    }
    paths
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_unicode_glyphs_have_strokes() {
        for text in ["R1", "µF", "Ω", "电路"] {
            assert!(!paths(text, 1.27, false).is_empty());
        }
        assert!(paths(" ", 1., true).is_empty());
    }
}
