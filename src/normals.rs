// Relief for remastered textures (roadmap phase 7): a normal map made from a texture's own picture, so that rivets,
// plank gaps and ropes catch the light. The height is the picture's brightness with its broad shading taken away (what
// stays is the small detail: a bright rivet is a bump, a dark gap a groove), lightly smoothed against noise; the slope
// of that height at the scale of the original's texels tilts the normal. A texture that repeats as a tile is read
// across its edges from its other side, so its normal map repeats without a seam too; any other is mirrored there.
//
// The map is in tangent space as glTF has it: red along u, green towards -v (up in the picture), blue out.

use image::RgbaImage;

/// The strength the tests use (the toolbox's is chosen with --relief; the owner found 8 far too strong: the relief
/// is guessed from the picture, so every crack and gap became a groove).
#[cfg(test)]
const STRENGTH: f32 = 8.0;

/// The normal map of `img`, a texture grown `scale` times from the original's (its detail is that coarse).
pub fn normal_map(img: &RgbaImage, scale: u32, strength: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let wrap = crate::remaster::tiles(img);
    let (wi, hi) = (w as usize, h as usize);
    let lum: Vec<f32> = img
        .pixels()
        .map(|p| {
            (0.299 * f32::from(p.0[0]) + 0.587 * f32::from(p.0[1]) + 0.114 * f32::from(p.0[2]))
                / 255.0
        })
        .collect();
    let scale = scale.max(1);
    // detail: lightly smoothed, less the broad shading around it
    let fine = blur(&lum, wi, hi, (scale / 2).max(1) as usize, wrap);
    let broad = blur(&lum, wi, hi, ((w.min(h) / 8).max(4)) as usize, wrap);
    let height: Vec<f32> = fine.iter().zip(&broad).map(|(f, b)| f - b).collect();
    let d = (scale / 2).max(1) as i64;
    let at = |x: i64, y: i64| -> f32 {
        let (x, y) = (address(x, wi, wrap), address(y, hi, wrap));
        height.get(y * wi + x).copied().unwrap_or(0.0)
    };
    // the slope per texel of the original, so the relief is the same at any scale
    let per = scale as f32 / (2 * d) as f32;
    let mut out = RgbaImage::new(w, h);
    for (x, y, p) in out.enumerate_pixels_mut() {
        let (x, y) = (i64::from(x), i64::from(y));
        let hx = (at(x + d, y) - at(x - d, y)) * per;
        let hy = (at(x, y + d) - at(x, y - d)) * per;
        // the surface rises where the picture brightens: the normal leans away from the rise (y down in the picture,
        // so green, which points up, takes +hy)
        let n = glam::Vec3::new(-strength * hx, strength * hy, 1.0).normalize_or(glam::Vec3::Z);
        let c = |v: f32| ((v * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        *p = image::Rgba([c(n.x), c(n.y), c(n.z), 255]);
    }
    out
}

/// A texel index `i` of a row or column `n` long: around to the other side for a tile, mirrored for anything else.
fn address(i: i64, n: usize, wrap: bool) -> usize {
    let n = n.max(1) as i64;
    let j = if wrap {
        i.rem_euclid(n)
    } else {
        let m = i.rem_euclid(2 * n);
        if m < n { m } else { 2 * n - 1 - m }
    };
    j as usize
}

/// A box blur of radius `r` in both directions, the edges read as `address` does (running sums: any radius is cheap).
fn blur(v: &[f32], w: usize, h: usize, r: usize, wrap: bool) -> Vec<f32> {
    let pass =
        |src: &[f32], len: usize, count: usize, get: &dyn Fn(usize, usize) -> usize| -> Vec<f32> {
            let mut out = vec![0.0f32; src.len()];
            let r = r as i64;
            let k = (2 * r + 1) as f32;
            for line in 0..count {
                let val = |i: i64| {
                    src.get(get(line, address(i, len, wrap)))
                        .copied()
                        .unwrap_or(0.0)
                };
                let mut sum: f32 = (-r..=r).map(val).sum();
                for i in 0..len as i64 {
                    if let Some(o) = out.get_mut(get(line, i as usize)) {
                        *o = sum / k;
                    }
                    sum += val(i + r + 1) - val(i - r);
                }
            }
            out
        };
    let rows = pass(v, w, h, &|line, i| line * w + i);
    pass(&rows, h, w, &|line, i| i * w + line)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grey square with a bright round bump in the middle.
    fn bump() -> RgbaImage {
        let mut img = RgbaImage::new(64, 64);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let d = ((x as f32 - 32.0).powi(2) + (y as f32 - 32.0).powi(2)).sqrt();
            let v = (120.0 + 100.0 * (1.0 - d / 10.0).max(0.0)) as u8;
            *p = image::Rgba([v, v, v, 255]);
        }
        img
    }

    #[test]
    fn a_bump_leans_its_sides_outwards() {
        let n = normal_map(&bump(), 1, STRENGTH);
        let px = |x: u32, y: u32| n.get_pixel(x, y).0;
        // left of the bump the normal leans left (red low), right of it right; above it up (green high), below down
        assert!(
            px(26, 32)[0] < 120 && px(38, 32)[0] > 136,
            "{:?} {:?}",
            px(26, 32),
            px(38, 32)
        );
        assert!(
            px(32, 26)[1] > 136 && px(32, 38)[1] < 120,
            "{:?} {:?}",
            px(32, 26),
            px(32, 38)
        );
        // far from it the surface is flat
        assert_eq!(px(2, 2), [128, 128, 255, 255]);
    }

    #[test]
    fn a_tile_gives_a_map_that_tiles() {
        // stripes that repeat across the tile's edges
        let mut tile = RgbaImage::new(32, 32);
        for (x, _, p) in tile.enumerate_pixels_mut() {
            let v = if ((x + 2) / 4) % 2 == 0 { 60 } else { 200 };
            *p = image::Rgba([v, v, v, 255]);
        }
        let n = normal_map(&tile, 2, STRENGTH);
        // across the left and right edges the map goes on as it does within: column 0 is column 8's twin
        for y in 0..32 {
            assert_eq!(n.get_pixel(0, y), n.get_pixel(8, y));
            assert_eq!(n.get_pixel(31, y), n.get_pixel(7, y));
        }
    }
}
