use tray_icon::Icon;

/// Builds a crisp 22x22 monochrome template icon for the macOS menu bar.
///
/// The icon uses solid black pixels with alpha transparency so macOS
/// automatically adapts it to Light and Dark menu bar appearances.
pub fn build_tray_icon() -> Result<Icon, tray_icon::BadIcon> {
    const SIZE: u32 = 22;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];

    // Draw a clean agent robot / terminal emblem:
    // - Rounded outer frame (3..19 x 4..18)
    // - Antenna dot at top center (10..12, 1..3)
    // - Two eyes inside (7..9, 9..11) and (13..15, 9..11)
    // - Mouth bar (8..14, 14..15)
    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut alpha = 0_u8;

            // Top antenna
            if (10..=11).contains(&x) && (1..=3).contains(&y) {
                alpha = 255;
            }

            // Outer rounded rectangle border (thickness 2px)
            let in_outer = (3..=18).contains(&x) && (4..=18).contains(&y);
            let is_corner = (x == 3 || x == 18) && (y == 4 || y == 18);
            let in_inner = (5..=16).contains(&x) && (6..=16).contains(&y);

            if in_outer && !is_corner && !in_inner {
                alpha = 255;
            }

            // Left & right eyes
            let left_eye = (7..=8).contains(&x) && (9..=11).contains(&y);
            let right_eye = (13..=14).contains(&x) && (9..=11).contains(&y);
            // Mouth line
            let mouth = (8..=13).contains(&x) && y == 14;

            if left_eye || right_eye || mouth {
                alpha = 255;
            }

            let idx = ((y * SIZE + x) * 4) as usize;
            rgba[idx] = 0;
            rgba[idx + 1] = 0;
            rgba[idx + 2] = 0;
            rgba[idx + 3] = alpha;
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE)
}
