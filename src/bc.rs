// Block compression for the game's compressed textures (the game's gputex.rs reads them; its open-format.md,
// "Compressed textures"): a picture and its whole mip chain as BC1 blocks (an opaque colour picture: 4 bits a pixel)
// or BC7 blocks of mode 6 (a picture with alpha, a normal map with its height: 8 bits a pixel). Our own encoder, no
// library: each 4 x 4 block's colours are fitted with a line (their main axis), the pixels given their places along
// it, the line's ends then solved again for those places (least squares) and rounded to what the format stores. The
// blocks of a level are made on every processor at once.

use gamedata::gputex::{BC7_WEIGHTS, Format, bc1_palette, bc7_mix};

/// The smallest picture worth compressing (pixels): below it the PNG takes next to no memory, and keeps every texel.
pub const MIN_PIXELS: u64 = 512 * 512;

/// The format a picture is stored in: BC7 when it has alpha or must stay fine (a normal map), else BC1.
pub fn format_for(rgba: &[u8], fine: bool) -> Format {
    if fine || rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255) {
        Format::Bc7
    } else {
        Format::Bc1
    }
}

/// The whole mip chain of an RGBA picture as blocks, the largest level first; none when the picture is not whole
/// blocks (sides multiples of four) or its bytes are not its size.
pub fn encode(width: u32, height: u32, rgba: &[u8], format: Format) -> Option<Vec<Vec<u8>>> {
    if width == 0
        || height == 0
        || !width.is_multiple_of(4)
        || !height.is_multiple_of(4)
        || format == Format::Rgba
    {
        return None;
    }
    if rgba.len()
        != (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?
    {
        return None;
    }
    let mut levels = Vec::new();
    let (mut w, mut h, mut px) = (width, height, rgba.to_vec());
    loop {
        levels.push(encode_level(w, h, &px, format));
        if w == 1 && h == 1 {
            break;
        }
        px = gamedata::gputex::halve(w, h, &px)?;
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    Some(levels)
}

/// One level's blocks, its rows of blocks shared among the processors.
fn encode_level(w: u32, h: u32, px: &[u8], format: Format) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let size = format.block_bytes();
    let mut out = vec![0u8; bw * bh * size];
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(bh)
        .max(1);
    let rows_each = bh.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, part) in out.chunks_mut(rows_each * bw * size).enumerate() {
            s.spawn(move || {
                for (i, block) in part.chunks_exact_mut(size).enumerate() {
                    let (bx, by) = (i % bw, k * rows_each + i / bw);
                    // the block's 16 pixels; past the picture's edge (the small levels) the edge's own
                    let mut c = [[0f32; 4]; 16];
                    for (j, p) in c.iter_mut().enumerate() {
                        let (x, y) = ((bx * 4 + j % 4).min(w - 1), (by * 4 + j / 4).min(h - 1));
                        if let Some(q) = px.get((y * w + x) * 4..(y * w + x) * 4 + 4) {
                            *p = [
                                f32::from(q[0]),
                                f32::from(q[1]),
                                f32::from(q[2]),
                                f32::from(q[3]),
                            ];
                        }
                    }
                    match format {
                        Format::Bc1 => block.copy_from_slice(&bc1_block(&c)),
                        _ => block.copy_from_slice(&bc7_block(&c)),
                    }
                }
            });
        }
    });
    out
}

/// The line through a block's colours: its two ends (the first the far end along the main axis), over `dims`
/// channels. Equal ends: the block is one colour.
fn fit_line(c: &[[f32; 4]; 16], dims: usize) -> ([f32; 4], [f32; 4]) {
    let mut mean = [0f32; 4];
    for p in c {
        for d in 0..dims {
            mean[d] += p[d] / 16.0;
        }
    }
    let mut cov = [[0f32; 4]; 4];
    let (mut lo, mut hi) = ([255f32; 4], [0f32; 4]);
    for p in c {
        for a in 0..dims {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
            for b in 0..dims {
                cov[a][b] += (p[a] - mean[a]) * (p[b] - mean[b]);
            }
        }
    }
    // the main axis by power iteration, begun from the box the colours lie in
    let mut axis = [0f32; 4];
    for d in 0..dims {
        axis[d] = hi[d] - lo[d];
    }
    for _ in 0..6 {
        let mut next = [0f32; 4];
        for a in 0..dims {
            for b in 0..dims {
                next[a] += cov[a][b] * axis[b];
            }
        }
        let len = next.iter().map(|v| v * v).sum::<f32>().sqrt();
        if len < 1e-6 {
            break;
        }
        for d in 0..dims {
            axis[d] = next[d] / len;
        }
    }
    let len = axis.iter().map(|v| v * v).sum::<f32>().sqrt();
    if len < 1e-6 {
        return (mean, mean);
    }
    let (mut tmin, mut tmax) = (f32::MAX, f32::MIN);
    for p in c {
        let t: f32 = (0..dims).map(|d| (p[d] - mean[d]) * axis[d] / len).sum();
        tmin = tmin.min(t);
        tmax = tmax.max(t);
    }
    let end = |t: f32| {
        let mut e = mean;
        for d in 0..dims {
            e[d] = (mean[d] + axis[d] / len * t).clamp(0.0, 255.0);
        }
        e
    };
    (end(tmax), end(tmin))
}

/// Each pixel's place along the line from `b` (0) to `a` (1).
fn places(c: &[[f32; 4]; 16], a: &[f32; 4], b: &[f32; 4], dims: usize) -> [f32; 16] {
    let mut d = [0f32; 4];
    let mut dd = 0f32;
    for k in 0..dims {
        d[k] = a[k] - b[k];
        dd += d[k] * d[k];
    }
    let mut out = [0f32; 16];
    if dd < 1e-6 {
        return out;
    }
    for (o, p) in out.iter_mut().zip(c) {
        *o = ((0..dims).map(|k| (p[k] - b[k]) * d[k]).sum::<f32>() / dd).clamp(0.0, 1.0);
    }
    out
}

/// The ends that fit the pixels best when each pixel is `weights[i]` of the way from `b` to `a` (least squares);
/// none when every pixel is at one place.
fn refit(c: &[[f32; 4]; 16], weights: &[f32; 16], dims: usize) -> Option<([f32; 4], [f32; 4])> {
    let (mut aa, mut ab, mut bb) = (0f32, 0f32, 0f32);
    let (mut ca, mut cb) = ([0f32; 4], [0f32; 4]);
    for (p, &w) in c.iter().zip(weights) {
        aa += w * w;
        ab += w * (1.0 - w);
        bb += (1.0 - w) * (1.0 - w);
        for d in 0..dims {
            ca[d] += w * p[d];
            cb[d] += (1.0 - w) * p[d];
        }
    }
    let det = aa * bb - ab * ab;
    if det.abs() < 1e-4 {
        return None;
    }
    let (mut a, mut b) = ([255f32; 4], [255f32; 4]);
    for d in 0..dims {
        a[d] = ((ca[d] * bb - cb[d] * ab) / det).clamp(0.0, 255.0);
        b[d] = ((cb[d] * aa - ca[d] * ab) / det).clamp(0.0, 255.0);
    }
    Some((a, b))
}

fn to565(c: &[f32; 4]) -> u16 {
    let q = |v: f32, max: f32| (v / 255.0 * max).round().clamp(0.0, max) as u16;
    (q(c[0], 31.0) << 11) | (q(c[1], 63.0) << 5) | q(c[2], 31.0)
}

/// A BC1 block, always in the four-colour mode (the first colour the larger) or of one colour.
fn bc1_block(c: &[[f32; 4]; 16]) -> [u8; 8] {
    let (mut a, mut b) = fit_line(c, 3);
    // the places along the line snapped to the four the format has, then the ends solved again for them
    let mut w = places(c, &a, &b, 3);
    for v in &mut w {
        *v = (*v * 3.0).round() / 3.0;
    }
    if let Some((ra, rb)) = refit(c, &w, 3) {
        (a, b) = (ra, rb);
    }
    let (mut c0, mut c1) = (to565(&a), to565(&b));
    if c0 < c1 {
        std::mem::swap(&mut c0, &mut c1);
    }
    let mut out = [0u8; 8];
    out[..2].copy_from_slice(&c0.to_le_bytes());
    out[2..4].copy_from_slice(&c1.to_le_bytes());
    if c0 == c1 {
        return out;
    }
    // each pixel the nearest of the four colours the card will make of these ends
    let palette = bc1_palette(c0, c1);
    let mut bits = 0u32;
    for (i, p) in c.iter().enumerate() {
        let mut best = (f32::MAX, 0u32);
        for (k, q) in palette.iter().enumerate() {
            let e: f32 = (0..3).map(|d| (p[d] - f32::from(q[d])).powi(2)).sum();
            if e < best.0 {
                best = (e, k as u32);
            }
        }
        bits |= best.1 << (2 * i);
    }
    out[4..].copy_from_slice(&bits.to_le_bytes());
    out
}

/// An end rounded to what BC7's mode 6 stores: 7 bits a channel and one low bit all four share.
fn to7777(c: &[f32; 4]) -> ([u8; 4], u8) {
    let mut best = (f32::MAX, [0u8; 4], 0u8);
    for p in 0..2u8 {
        let mut q = [0u8; 4];
        let mut e = 0f32;
        for d in 0..4 {
            let v = ((c[d] - f32::from(p)) / 2.0).round().clamp(0.0, 127.0) as u8;
            q[d] = (v << 1) | p;
            e += (f32::from(q[d]) - c[d]).powi(2);
        }
        if e < best.0 {
            best = (e, q, p);
        }
    }
    (best.1, best.2)
}

/// A BC7 block of mode 6: one line through red, green, blue and alpha, sixteen places along it.
fn bc7_block(c: &[[f32; 4]; 16]) -> [u8; 16] {
    let (mut a, mut b) = fit_line(c, 4);
    let mut w = places(c, &a, &b, 4);
    for v in &mut w {
        let k = (*v * 15.0).round() as usize;
        *v = BC7_WEIGHTS.get(k).copied().unwrap_or(64) as f32 / 64.0;
    }
    if let Some((ra, rb)) = refit(c, &w, 4) {
        (a, b) = (ra, rb);
    }
    // the format's first end is where the indices begin (weight 0)
    let ((mut e0, mut p0), (mut e1, mut p1)) = (to7777(&b), to7777(&a));
    let ef = |e: &[u8; 4]| {
        [
            f32::from(e[0]),
            f32::from(e[1]),
            f32::from(e[2]),
            f32::from(e[3]),
        ]
    };
    let along = places(c, &ef(&e1), &ef(&e0), 4);
    let mut index = [0u32; 16];
    for (i, p) in c.iter().enumerate() {
        // the place nearest along the line, and its neighbours tried against what the card will make
        let guess = (along[i] * 15.0).round() as i32;
        let mut best = (f32::MAX, 0u32);
        for k in (guess - 1).max(0)..=(guess + 1).min(15) {
            let weight = BC7_WEIGHTS[k as usize];
            let e: f32 = (0..4)
                .map(|d| (p[d] - f32::from(bc7_mix(e0[d], e1[d], weight))).powi(2))
                .sum();
            if e < best.0 {
                best = (e, k as u32);
            }
        }
        index[i] = best.1;
    }
    // the first pixel's index is stored without its top bit: the ends change places when it has one
    if index[0] >= 8 {
        std::mem::swap(&mut e0, &mut e1);
        std::mem::swap(&mut p0, &mut p1);
        for k in &mut index {
            *k = 15 - *k;
        }
    }
    let mut bits: u128 = 0x40;
    for d in 0..4 {
        bits |= u128::from(e0[d] >> 1) << (7 + d * 14);
        bits |= u128::from(e1[d] >> 1) << (14 + d * 14);
    }
    bits |= u128::from(p0) << 63;
    bits |= u128::from(p1) << 64;
    let mut at = 65;
    for (i, k) in index.iter().enumerate() {
        bits |= u128::from(*k) << at;
        at += if i == 0 { 3 } else { 4 };
    }
    bits.to_le_bytes()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use gamedata::gputex::{decode_level, read, write};

    /// A picture with what textures have: a smooth gradient, a hard edge, grain, and (with `alpha`) a soft hole.
    fn picture(side: u32, alpha: bool) -> Vec<u8> {
        let mut out = Vec::new();
        let mut seed = 12345u32;
        for y in 0..side {
            for x in 0..side {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let grain = (seed >> 28) as f32;
                let edge = if (x / 16 + y / 16) % 2 == 0 {
                    40.0
                } else {
                    0.0
                };
                let (fx, fy) = (x as f32 / side as f32, y as f32 / side as f32);
                let a = if alpha { (fx * 255.0) as u8 } else { 255 };
                out.extend_from_slice(&[
                    (60.0 + 150.0 * fx + edge + grain) as u8,
                    (40.0 + 120.0 * fy + edge + grain) as u8,
                    (90.0 + 60.0 * fx * fy + grain) as u8,
                    a,
                ]);
            }
        }
        out
    }

    /// The signal-to-noise ratio of what the card draws against the picture, in decibels.
    fn psnr(a: &[u8], b: &[u8]) -> f64 {
        let mse = a
            .iter()
            .zip(b)
            .map(|(x, y)| (f64::from(*x) - f64::from(*y)).powi(2))
            .sum::<f64>()
            / a.len() as f64;
        10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10()
    }

    #[test]
    fn a_picture_survives_both_formats_and_the_game_reads_the_file() {
        // the bounds come from the formats: BC1 keeps 5-6-5 bits and four colours a block, BC7's mode 6 sixteen
        for (alpha, format, least) in [
            (false, Format::Bc1, 32.0),
            (false, Format::Bc7, 40.0),
            (true, Format::Bc7, 40.0),
        ] {
            let px = picture(64, alpha);
            let levels = encode(64, 64, &px, format).unwrap();
            assert_eq!(levels.len(), 7);
            let file = write(format, 64, 64, (1, 2), &levels).unwrap();
            assert!(read(&file).is_ok());
            let back = decode_level(format, 64, 64, &levels[0]).unwrap();
            let q = psnr(&px, &back);
            assert!(q > least, "{format:?} alpha {alpha}: {q:.1} dB");
        }
        assert_eq!(format_for(&picture(8, false), false), Format::Bc1);
        assert_eq!(format_for(&picture(8, true), false), Format::Bc7);
        assert_eq!(format_for(&picture(8, false), true), Format::Bc7);
    }

    #[test]
    fn flat_and_two_colour_blocks_are_exact_enough() {
        // one colour: within the format's rounding; black and white: exact in BC1, within one in BC7's mode 6 (an
        // end's four channels share their lowest bit: black with full alpha cannot be both)
        let flat: Vec<u8> = [200u8, 100, 50, 255].repeat(16);
        for format in [Format::Bc1, Format::Bc7] {
            let l = encode(4, 4, &flat, format).unwrap();
            let back = decode_level(format, 4, 4, &l[0]).unwrap();
            assert!(
                flat.iter().zip(&back).all(|(a, b)| a.abs_diff(*b) <= 4),
                "{format:?} {back:?}"
            );
            let mut two = [0u8, 0, 0, 255].repeat(8);
            two.extend([255u8; 4].repeat(8));
            let l = encode(4, 4, &two, format).unwrap();
            let back = decode_level(format, 4, 4, &l[0]).unwrap();
            let off = u8::from(format == Format::Bc7);
            assert!(
                two.iter().zip(&back).all(|(a, b)| a.abs_diff(*b) <= off),
                "{format:?} {back:?}"
            );
        }
        // not whole blocks, or bytes that are not the picture's
        assert!(encode(6, 4, &[0; 96], Format::Bc1).is_none());
        assert!(encode(4, 4, &[0; 3], Format::Bc1).is_none());
    }
}
