// Rounder shapes for the remastered look (roadmap phase 7): the original's round things are low-poly, and most of the
// levels' geometry is shaded flat, one normal per face. Faces that meet at a shallow angle (under 40 degrees: a 9-sided
// or rounder shape) are taken as one smooth surface and their corners get the surface's normal; each curved mesh is
// then refined as curved point-normal triangles: every triangle becomes a cubic patch through its corners, its edges
// bent to meet the corners' normals, cut into smaller triangles. Flat faces stay flat and sharp edges (a box's) stay
// straight. The refined mesh is written into the level as the mesh's look, which the game draws; its physics keeps the
// mesh itself, so runs and replays are the same.
//
// An edge is bent the same way for every triangle on it, so the refined mesh has no cracks: where two faces meet at
// a crease (the same corners, different normals, such as a disc's top and its rim), the edge takes the normals that
// bend it most, so a flat top follows its round side. Edges bent at one end only bend just a little (see `bend`).

use glam::Vec3;
use level::{Level, Mesh, Part};
use std::collections::HashMap;

/// The most a mesh's edges are cut into: enough for the original's 8- to 24-sided round shapes.
const MAX_CUTS: u32 = 8;
/// The bend that takes one more cut: about 7 degrees (a 20-degree piece of a ring is cut in three).
const CUT_ANGLE: f32 = 0.12;
/// The most vertices a refined mesh may have (a mesh past it is drawn as it is).
const MAX_VERTICES: usize = 1 << 22;
/// Faces meeting at less than this angle are one smooth surface (cos 40 degrees).
const SMOOTH_COS: f32 = 0.766;
/// The most an edge may bow out of its straight line, in level units: round things of the original bow a few tenths at
/// most, while a long ramp between two kinks would rise off the floor the ball rolls on.
const MAX_BOW: f32 = 0.35;

/// Gives each curved mesh of the level its refined look (made from the mesh itself; an older look is replaced or
/// dropped). Returns how many meshes have one.
pub fn looks(level: &mut Level) -> usize {
    let mut n = 0;
    for m in &mut level.meshes {
        m.look = None;
        m.look = mesh(m).map(Box::new);
        n += usize::from(m.look.is_some());
    }
    n
}

/// A position's key, to find the corners triangles share (their vertices may be split by normals or coordinates).
fn key(p: Vec3) -> [i64; 3] {
    let q = |v: f32| (f64::from(v) * 4096.0).round() as i64;
    [q(p.x), q(p.y), q(p.z)]
}

/// One edge's bend: the inner control points of its cubic, from its lower-keyed corner to the other.
#[derive(Clone, Copy)]
struct Bend {
    near_lo: Vec3,
    near_hi: Vec3,
    /// How much the edge bends (the sum of its corners' offsets), to keep the most bent.
    amount: f32,
    /// The angle between the normals that bend it.
    angle: f32,
}

/// The least share of one corner's offset in the other's for an edge to bend like an arc. An edge whose normals tilt
/// at one end only (a corner between two straight runs, or where a straight rail meets its curve) bends lopsided: it
/// may, but only a little ([`MAX_LOPSIDED_BOW`]), so a corner is rounded while a long rail does not bend into a wave.
const ARC_LIKE: f32 = 0.2;
/// An edge longer than this many times the edge it goes on into is a straight run, never bent.
const RUN_RATIO: f32 = 2.5;
/// The most a lopsided edge may bow, in level units.
const MAX_LOPSIDED_BOW: f32 = 0.1;

fn bend(lo: Vec3, hi: Vec3, n_lo: Vec3, n_hi: Vec3) -> Bend {
    let w_lo = (hi - lo).dot(n_lo);
    let w_hi = (lo - hi).dot(n_hi);
    let (least, most) = (w_lo.abs().min(w_hi.abs()), w_lo.abs().max(w_hi.abs()));
    // the ends bend the edge the same way (an end that hardly tilts counts as not bending it); an S is never bent
    let one_way = w_lo * w_hi > 0.0 || least <= 0.2 * most;
    let lopsided = least < ARC_LIKE * most;
    let bow = (w_lo.abs() + w_hi.abs()) / 8.0;
    if !one_way || most == 0.0 || (lopsided && bow > MAX_LOPSIDED_BOW) {
        return Bend {
            near_lo: lo.lerp(hi, 1.0 / 3.0),
            near_hi: hi.lerp(lo, 1.0 / 3.0),
            amount: 0.0,
            angle: 0.0,
        };
    }
    Bend {
        near_lo: (2.0 * lo + hi - w_lo * n_lo) / 3.0,
        near_hi: (2.0 * hi + lo - w_hi * n_hi) / 3.0,
        amount: w_lo.abs() + w_hi.abs(),
        angle: n_lo.dot(n_hi).clamp(-1.0, 1.0).acos(),
    }
}

/// Each triangle's corner normals: the mesh's own where it is shaded smooth, else the normal of the smooth surface the
/// face belongs to (the faces around the corner that meet it at a shallow angle, weighted by their area).
fn corner_normals(m: &Mesh, tris: &[[u32; 3]]) -> Option<Vec<[Vec3; 3]>> {
    let pos = |i: u32| m.positions.get(i as usize).map(|&p| Vec3::from(p));
    // each face's normal, and around each corner the faces there with their angle at it (weighting by the angle keeps
    // the normal the same however a quad was split into triangles)
    let mut faces = Vec::with_capacity(tris.len());
    let mut around: HashMap<[i64; 3], Vec<(usize, f32)>> = HashMap::new();
    for (f, t) in tris.iter().enumerate() {
        let p = [pos(t[0])?, pos(t[1])?, pos(t[2])?];
        faces.push((p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero());
        for c in 0..3 {
            let (a, b, o) = (p[c], p[(c + 1) % 3], p[(c + 2) % 3]);
            let angle = (b - a).angle_between(o - a);
            around
                .entry(key(a))
                .or_default()
                .push((f, if angle.is_finite() { angle } else { 0.0 }));
        }
    }
    let mut out = Vec::with_capacity(tris.len());
    for (f, t) in tris.iter().enumerate() {
        let own = *faces.get(f)?;
        let mut corners = [own; 3];
        for (c, &i) in corners.iter_mut().zip(t) {
            let given = m
                .normals
                .get(i as usize)
                .map(|&n| Vec3::from(n).normalize_or_zero())
                .unwrap_or(own);
            if given != Vec3::ZERO && given.dot(own) < 0.999 {
                // shaded smooth by its maker
                *c = given;
                continue;
            }
            let sum: Vec3 = around
                .get(&key(pos(i)?))
                .into_iter()
                .flatten()
                .filter_map(|&(g, angle)| faces.get(g).map(|n| (*n, angle)))
                .filter(|(n, _)| n.dot(own) >= SMOOTH_COS)
                .map(|(n, angle)| n * angle)
                .sum();
            *c = sum.normalize_or(own);
        }
        out.push(corners);
    }
    Some(out)
}

/// The bend of the edge lo..hi in the line before, lo, hi, after: Catmull-Rom tangents, a third of the edge long;
/// none when it would bow out more than an arc of the original's round things does (MAX_BOW), or the line is not a
/// curve at all.
fn curve(before: Vec3, lo: Vec3, hi: Vec3, after: Vec3) -> Option<Bend> {
    let len = lo.distance(hi);
    let (t_lo, t_hi) = (
        (hi - before).normalize_or_zero(),
        (after - lo).normalize_or_zero(),
    );
    if len <= 0.0 || t_lo == Vec3::ZERO || t_hi == Vec3::ZERO {
        return None;
    }
    let near_lo = lo + t_lo * (len / 3.0);
    let near_hi = hi - t_hi * (len / 3.0);
    // how far the controls stand off the chord: an arc's middle bows about three quarters of that
    let chord = (hi - lo) / len;
    let off = |p: Vec3| {
        let d = p - lo;
        (d - chord * d.dot(chord)).length()
    };
    let bow = 0.75 * off(near_lo).max(off(near_hi));
    if bow > MAX_BOW || bow < 1e-4 * len {
        return None;
    }
    Some(Bend {
        near_lo,
        near_hi,
        amount: 8.0 * bow,
        angle: t_lo.dot(t_hi).clamp(-1.0, 1.0).acos(),
    })
}

fn cubic(a: Vec3, b: Vec3, c: Vec3, d: Vec3, t: f32) -> Vec3 {
    let s = 1.0 - t;
    a * (s * s * s) + b * (3.0 * s * s * t) + c * (3.0 * s * t * t) + d * (t * t * t)
}

/// The mesh refined, or None when it has no curved edge (or would grow too large).
pub fn mesh(m: &Mesh) -> Option<Mesh> {
    let pos = |i: u32| m.positions.get(i as usize).map(|&p| Vec3::from(p));
    let tris: Vec<[u32; 3]> = m
        .parts
        .iter()
        .flat_map(|p| p.indices.as_chunks::<3>().0.iter().copied())
        .collect();
    let corners = corner_normals(m, &tris)?;
    // every edge's most bending normals
    // the corners each corner is joined to by an edge, to find how a line of edges goes on past a corner
    let mut joined: HashMap<[i64; 3], Vec<Vec3>> = HashMap::new();
    for t in &tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (Some(pa), Some(pb)) = (pos(a), pos(b)) else {
                return None;
            };
            joined.entry(key(pa)).or_default().push(pb);
            joined.entry(key(pb)).or_default().push(pa);
        }
    }
    // The edge from `from` to `at` goes on past `at` along the edge most nearly in line with it: its length, if any
    // goes on within 45 degrees.
    // The corner the line from `from` through `at` goes on to: among the edges at `at` within 45 degrees of the line,
    // the one most in line and most like the edge in length (a ring goes on by its next piece, not by the shorter
    // edge of something joined to it, as a pedestal on a platform's rim).
    let beyond = |from: Vec3, at: Vec3| -> Option<Vec3> {
        let dir = (at - from).normalize_or_zero();
        let len = from.distance(at).max(1e-6);
        let score = |p: Vec3| {
            let along = (p - at).normalize_or_zero().dot(dir);
            let ratio = (p.distance(at) / len).max(1e-6).ln().abs();
            (along, along - 0.5 * ratio)
        };
        joined
            .get(&key(at))?
            .iter()
            .map(|&p| (p, score(p)))
            .filter(|&(_, (along, _))| along > 0.7)
            .max_by(|x, y| x.1.1.total_cmp(&y.1.1))
            .map(|(p, _)| p)
    };
    let goes_on =
        |from: Vec3, at: Vec3| -> Option<f32> { beyond(from, at).map(|p| p.distance(at)) };
    let mut bends: HashMap<([i64; 3], [i64; 3]), Bend> = HashMap::new();
    let mut size = 0.0f32;
    for (t, n) in tris.iter().zip(&corners) {
        for (a, b, na, nb) in [
            (t[0], t[1], n[0], n[1]),
            (t[1], t[2], n[1], n[2]),
            (t[2], t[0], n[2], n[0]),
        ] {
            let (Some(pa), Some(pb)) = (pos(a), pos(b)) else {
                return None;
            };
            let (ka, kb) = (key(pa), key(pb));
            if ka == kb {
                continue;
            }
            size = size.max(pa.distance(pb));
            let (lo, hi, n_lo, n_hi, k) = if ka < kb {
                (pa, pb, na, nb, (ka, kb))
            } else {
                (pb, pa, nb, na, (kb, ka))
            };
            let mut e = bend(lo, hi, n_lo, n_hi);
            // a round thing is cut into pieces of about one length; an edge far longer than the one it goes on into
            // is a straight run meeting a curve (a rail before its bend), which must stay straight
            let len = lo.distance(hi);
            let run = [goes_on(hi, lo), goes_on(lo, hi)]
                .into_iter()
                .flatten()
                .any(|next| len > RUN_RATIO * next);
            if run || e.amount / 8.0 > MAX_BOW {
                e = bend(lo, hi, Vec3::ZERO, Vec3::ZERO);
            } else if e.amount > 0.0
                && let (Some(before), Some(after)) = (beyond(hi, lo), beyond(lo, hi))
            {
                // A bent edge in a line that goes on smoothly at both ends (a ring of a round thing) follows the
                // line itself: its ends' tangents point from the corner before to the corner after (Catmull-Rom).
                // Normals tilted by the faces of what joins the ring (a pedestal on a platform's rim) would bend
                // it lopsided and leave the round thing a polygon.
                if let Some(c) = curve(before, lo, hi, after) {
                    e = c;
                }
            }
            let keep = bends.get(&k).is_some_and(|old| old.amount >= e.amount);
            if !keep {
                bends.insert(k, e);
            }
        }
    }
    let angle = bends
        .values()
        .filter(|b| b.amount > 1e-3 * size)
        .map(|b| b.angle)
        .fold(0.0f32, f32::max);
    let cuts = ((angle / CUT_ANGLE).ceil() as u32).min(MAX_CUTS);
    if cuts < 2 {
        return None;
    }
    let per_tri = ((cuts + 1) * (cuts + 2) / 2) as usize;
    if tris.len().checked_mul(per_tri)? > MAX_VERTICES {
        return None;
    }

    let mut out = Mesh {
        name: m.name.clone(),
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        parts: Vec::new(),
        look: None,
        channels: m
            .channels
            .iter()
            .map(|c| level::Channel {
                uvs: Vec::new(),
                ..c.clone()
            })
            .collect(),
    };
    // A point of the edge from corner a to corner b, a fraction t of the way, the same from either side.
    let edge_point = |pa: Vec3, pb: Vec3, t: f32| -> Vec3 {
        let (ka, kb) = (key(pa), key(pb));
        let (lo, hi, t) = if ka < kb {
            (pa, pb, t)
        } else {
            (pb, pa, 1.0 - t)
        };
        match bends.get(&if ka < kb { (ka, kb) } else { (kb, ka) }) {
            Some(b) => cubic(lo, b.near_lo, b.near_hi, hi, t),
            None => lo.lerp(hi, t),
        }
    };
    // The control point next to corner a on its edge to b.
    let near = |pa: Vec3, pb: Vec3| -> Vec3 {
        let (ka, kb) = (key(pa), key(pb));
        match bends.get(&if ka < kb { (ka, kb) } else { (kb, ka) }) {
            Some(b) if ka < kb => b.near_lo,
            Some(b) if ka > kb => b.near_hi,
            _ => pa.lerp(pb, 1.0 / 3.0),
        }
    };
    let mut corner = corners.iter();
    for part in &m.parts {
        let mut indices = Vec::new();
        for t in part.indices.as_chunks::<3>().0 {
            let [i0, i1, i2] = [t[0], t[1], t[2]];
            let (Some(p0), Some(p1), Some(p2)) = (pos(i0), pos(i1), pos(i2)) else {
                return None;
            };
            // the shading follows the smooth surface the face is on (a face on none keeps its own normal)
            let [n0, n1, n2] = *corner.next()?;
            let (b210, b120, b021, b012, b102, b201) = (
                near(p0, p1),
                near(p1, p0),
                near(p1, p2),
                near(p2, p1),
                near(p2, p0),
                near(p0, p2),
            );
            let e = (b210 + b120 + b021 + b012 + b102 + b201) / 6.0;
            let v = (p0 + p1 + p2) / 3.0;
            let b111 = e + (e - v) / 2.0;
            let uv = |i: u32, set: Option<&Vec<[f32; 2]>>| {
                glam::Vec2::from(
                    set.unwrap_or(&m.uvs)
                        .get(i as usize)
                        .copied()
                        .unwrap_or_default(),
                )
            };
            let base = u32::try_from(out.positions.len()).ok()?;
            let n = cuts as f32;
            // the points row by row: i steps of corner 0, j of corner 1, the rest of corner 2
            for i in (0..=cuts).rev() {
                for j in 0..=(cuts - i) {
                    let k = cuts - i - j;
                    let (u, w, s) = (i as f32 / n, j as f32 / n, k as f32 / n);
                    let p = if k == 0 {
                        edge_point(p0, p1, w)
                    } else if i == 0 {
                        edge_point(p1, p2, s)
                    } else if j == 0 {
                        edge_point(p2, p0, u)
                    } else {
                        p0 * (u * u * u)
                            + p1 * (w * w * w)
                            + p2 * (s * s * s)
                            + b210 * (3.0 * u * u * w)
                            + b120 * (3.0 * u * w * w)
                            + b021 * (3.0 * w * w * s)
                            + b012 * (3.0 * w * s * s)
                            + b102 * (3.0 * s * s * u)
                            + b201 * (3.0 * s * u * u)
                            + b111 * (6.0 * u * w * s)
                    };
                    let normal = (n0 * u + n1 * w + n2 * s).normalize_or(n0);
                    out.positions.push(p.to_array());
                    out.normals.push(normal.to_array());
                    out.uvs
                        .push((uv(i0, None) * u + uv(i1, None) * w + uv(i2, None) * s).to_array());
                    for (c, src) in out.channels.iter_mut().zip(&m.channels) {
                        let set = Some(&src.uvs);
                        c.uvs
                            .push((uv(i0, set) * u + uv(i1, set) * w + uv(i2, set) * s).to_array());
                    }
                }
            }
            // the index of point (i, j) in the rows above
            let at = |i: u32, j: u32| -> u32 {
                let rows_before = cuts - i;
                base + rows_before * (rows_before + 1) / 2 + j
            };
            for i in (1..=cuts).rev() {
                for j in 0..=(cuts - i) {
                    // the triangle pointing to corner 0, then the one beside it, in the source's winding
                    indices.extend([at(i, j), at(i - 1, j + 1), at(i - 1, j)]);
                    if j < cuts - i {
                        indices.extend([at(i, j), at(i, j + 1), at(i - 1, j + 1)]);
                    }
                }
            }
        }
        out.parts.push(Part {
            material: part.material,
            indices,
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ring of `sides` around the y axis, its side shaded smooth (normals pointing out), top shaded flat.
    fn cylinder(sides: u32) -> Mesh {
        let mut m = Mesh {
            name: "c".into(),
            ..Mesh::default()
        };
        let mut side = Vec::new();
        let mut top = Vec::new();
        for s in 0..sides {
            let a = s as f32 / sides as f32 * std::f32::consts::TAU;
            let d = Vec3::new(a.cos(), 0.0, a.sin());
            for y in [0.0, 1.0] {
                m.positions.push((d + Vec3::Y * y).to_array());
                m.normals.push(d.to_array());
                m.uvs.push([0.0; 2]);
            }
        }
        for s in 0..sides {
            let (a, b) = (2 * s, 2 * ((s + 1) % sides));
            side.extend([a, a + 1, b + 1, a, b + 1, b]);
        }
        let first = m.positions.len() as u32;
        for s in 0..sides {
            let a = s as f32 / sides as f32 * std::f32::consts::TAU;
            m.positions.push([a.cos(), 1.0, a.sin()]);
            m.normals.push([0.0, 1.0, 0.0]);
            m.uvs.push([0.0; 2]);
        }
        for s in 1..sides - 1 {
            top.extend([first, first + s + 1, first + s]);
        }
        m.parts = vec![
            Part {
                material: None,
                indices: side,
            },
            Part {
                material: None,
                indices: top,
            },
        ];
        m
    }

    #[test]
    fn a_round_side_grows_rounder_and_its_flat_top_follows() {
        let c = cylinder(12);
        let r = mesh(&c).expect("the side is curved");
        // every point stays within the round shape and the refined edge reaches nearer the circle than the chords
        let mut worst_before = 1.0f32;
        for t in 0..=20 {
            let a = Vec3::from(c.positions[0]);
            let b = Vec3::from(c.positions[2]);
            worst_before = worst_before.min(a.lerp(b, t as f32 / 20.0).length());
        }
        let mut worst_after = 1.0f32;
        for p in &r.positions {
            let p = Vec3::from(*p);
            assert!(
                Vec3::new(p.x, 0.0, p.z).length() < 1.01,
                "a point bulges out: {p}"
            );
        }
        // the side's points, nearer the circle than its chords were
        for &i in &r.parts[0].indices {
            let p = Vec3::from(r.positions[i as usize]);
            worst_after = worst_after.min(Vec3::new(p.x, 0.0, p.z).length());
        }
        assert!(
            worst_after > worst_before + 0.01,
            "{worst_after} vs {worst_before}"
        );
        for &i in &r.parts[1].indices {
            let p = Vec3::from(r.positions[i as usize]);
            assert!((p.y - 1.0).abs() < 1e-4, "the flat top bends: {p}");
        }
        // the top's rim and the side's top edge are the same curve: no crack
        let rim = |part: &Part| -> Vec<[i64; 3]> {
            let mut v: Vec<_> = part
                .indices
                .iter()
                .map(|&i| Vec3::from(r.positions[i as usize]))
                .filter(|p| p.y > 0.5 && Vec3::new(p.x, 0.0, p.z).length() > 0.995)
                .map(key)
                .collect();
            v.sort();
            v.dedup();
            v
        };
        let side_rim = rim(&r.parts[0]);
        assert!(rim(&r.parts[1]).iter().all(|k| side_rim.contains(k)));
    }

    #[test]
    fn a_flat_shaded_round_side_is_found_and_rounded() {
        let mut c = cylinder(16);
        // shaded flat, as the levels are: every face its own normal, so no vertex is shared
        let mut flat = Mesh {
            name: "f".into(),
            ..Mesh::default()
        };
        for part in &c.parts {
            let mut indices = Vec::new();
            for t in part.indices.chunks_exact(3) {
                let p: Vec<Vec3> = t
                    .iter()
                    .map(|&i| Vec3::from(c.positions[i as usize]))
                    .collect();
                let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize();
                for q in p {
                    indices.push(flat.positions.len() as u32);
                    flat.positions.push(q.to_array());
                    flat.normals.push(n.to_array());
                    flat.uvs.push([0.0; 2]);
                }
            }
            flat.parts.push(Part {
                material: None,
                indices,
            });
        }
        c = flat;
        let r = mesh(&c).expect("the side is found curved");
        let mut nearest = 1.0f32;
        for &i in &r.parts[0].indices {
            let p = Vec3::from(r.positions[i as usize]);
            nearest = nearest.min(Vec3::new(p.x, 0.0, p.z).length());
        }
        // a 16-gon's chords come to 0.981 of the radius
        assert!(nearest > 0.99, "{nearest}");
        for &i in &r.parts[1].indices {
            let p = Vec3::from(r.positions[i as usize]);
            assert!((p.y - 1.0).abs() < 1e-4 && Vec3::new(p.x, 0.0, p.z).length() < 1.01);
        }
    }

    #[test]
    fn flat_shaded_meshes_stay_as_they_are() {
        let mut box_face = Mesh::default();
        box_face.positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ];
        box_face.normals = vec![[0.0, 1.0, 0.0]; 4];
        box_face.uvs = vec![[0.0; 2]; 4];
        box_face.parts = vec![Part {
            material: None,
            indices: vec![0, 2, 1, 0, 3, 2],
        }];
        assert!(mesh(&box_face).is_none());
    }
}
