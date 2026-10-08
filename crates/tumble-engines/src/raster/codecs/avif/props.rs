//! The primary item's transform properties from the HEIF container
//! (ISO/IEC 23008-12): clean aperture (`clap`), rotation (`irot`) and
//! mirroring (`imir`). `avif-parse` does not expose these.
//!
//! Only the boxes on the path meta -> pitm / iprp -> ipco + ipma are read.
//! Anything malformed yields no transforms rather than an error, since the
//! image itself still decodes.

/// A display transform, in the order the file associates them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    /// Crop to this rectangle: left, top, width, height.
    Crop { x: u32, y: u32, width: u32, height: u32 },
    /// Rotate anti-clockwise by `quarter_turns` x 90 degrees.
    Rotate { quarter_turns: u8 },
    /// `vertical` mirrors top-bottom; otherwise left-right.
    Mirror { vertical: bool },
}

/// One ISO-BMFF box: its four-character type and payload.
struct Bmff<'a> {
    kind: [u8; 4],
    body: &'a [u8],
}

/// Iterates the boxes laid end to end in `data`.
fn boxes(mut data: &[u8]) -> impl Iterator<Item = Bmff<'_>> {
    std::iter::from_fn(move || {
        if data.len() < 8 {
            return None;
        }
        let size32 = u32::from_be_bytes(data[0..4].try_into().ok()?) as u64;
        let kind: [u8; 4] = data[4..8].try_into().ok()?;
        let (header, size) = match size32 {
            0 => (8, data.len() as u64),
            1 => (16, u64::from_be_bytes(data.get(8..16)?.try_into().ok()?)),
            n => (8, n),
        };
        let size = usize::try_from(size).ok()?;
        if size < header || size > data.len() {
            return None;
        }
        let body = &data[header..size];
        data = &data[size..];
        Some(Bmff { kind, body })
    })
}

fn find<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data).find(|b| &b.kind == kind).map(|b| b.body)
}

fn be_u16(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?)))
}

fn be_u32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// Transforms for the primary item of an image `width` x `height` (as
/// coded, before any transform).
pub fn transforms(file: &[u8], width: u32, height: u32) -> Vec<Transform> {
    parse(file, width, height).unwrap_or_default()
}

fn parse(file: &[u8], width: u32, height: u32) -> Option<Vec<Transform>> {
    // `meta` and `pitm` are full boxes: skip version and flags.
    let meta = find(file, b"meta")?.get(4..)?;
    let pitm = find(meta, b"pitm")?;
    let primary = if pitm[0] == 0 { be_u16(pitm, 4)? } else { be_u32(pitm, 4)? };

    let iprp = find(meta, b"iprp")?;
    let properties: Vec<Bmff> = boxes(find(iprp, b"ipco")?).collect();
    let ipma = find(iprp, b"ipma")?;
    let (version, flags) = (ipma[0], ipma[3]);
    let entries = be_u32(ipma, 4)?;
    let mut at = 8;
    for _ in 0..entries {
        let item = if version < 1 {
            at += 2;
            be_u16(ipma, at - 2)?
        } else {
            at += 4;
            be_u32(ipma, at - 4)?
        };
        let count = *ipma.get(at)?;
        at += 1;
        let mut indices = Vec::with_capacity(count as usize);
        for _ in 0..count {
            // The top bit marks "essential"; the rest is a 1-based index.
            let index = if flags & 1 != 0 {
                at += 2;
                be_u16(ipma, at - 2)? & 0x7FFF
            } else {
                at += 1;
                u32::from(*ipma.get(at - 1)? & 0x7F)
            };
            indices.push(index);
        }
        if item == primary {
            return Some(
                indices
                    .into_iter()
                    .filter_map(|i| properties.get((i as usize).checked_sub(1)?))
                    .filter_map(|p| property(p, width, height))
                    .collect(),
            );
        }
    }
    Some(Vec::new())
}

fn property(p: &Bmff, width: u32, height: u32) -> Option<Transform> {
    match &p.kind {
        b"irot" => Some(Transform::Rotate { quarter_turns: p.body.first()? & 0b11 }),
        // Mode 0 flips top-bottom, 1 left-right. Older spec text called this an
        // "axis" and read ambiguously; this follows libavif and the Link-U samples.
        b"imir" => Some(Transform::Mirror { vertical: p.body.first()? & 1 == 0 }),
        b"clap" => clean_aperture(p.body, width, height),
        _ => None,
    }
}

/// `clap` holds the crop size and the offset of its centre from the image
/// centre, each as a fraction (numerator, denominator).
fn clean_aperture(b: &[u8], width: u32, height: u32) -> Option<Transform> {
    let frac = |i: usize, signed: bool| -> Option<f64> {
        let n = be_u32(b, i * 8)?;
        let d = be_u32(b, i * 8 + 4)?;
        if d == 0 {
            return None;
        }
        let n = if signed { f64::from(n as i32) } else { f64::from(n) };
        Some(n / f64::from(d))
    };
    let (cw, ch) = (frac(0, false)?, frac(1, false)?);
    let (off_x, off_y) = (frac(2, true)?, frac(3, true)?);
    let left = (f64::from(width) - cw) / 2.0 + off_x;
    let top = (f64::from(height) - ch) / 2.0 + off_y;
    let whole = |v: f64| v.fract().abs() < 1e-9;
    if !(whole(cw) && whole(ch) && whole(left) && whole(top)) || cw < 1.0 || ch < 1.0 {
        return None;
    }
    if left < 0.0 || top < 0.0 || left + cw > f64::from(width) || top + ch > f64::from(height) {
        return None;
    }
    Some(Transform::Crop { x: left as u32, y: top as u32, width: cw as u32, height: ch as u32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn file(props: &[Vec<u8>], assoc: &[u8]) -> Vec<u8> {
        let pitm = bx(b"pitm", &[0, 0, 0, 0, 0, 1]);
        let ipco = bx(b"ipco", &props.concat());
        let mut ipma = vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 1, assoc.len() as u8];
        ipma.extend_from_slice(assoc);
        let iprp = bx(b"iprp", &[ipco, bx(b"ipma", &ipma)].concat());
        let meta = bx(b"meta", &[vec![0, 0, 0, 0], pitm, iprp].concat());
        [bx(b"ftyp", b"avif"), meta].concat()
    }

    #[test]
    fn reads_rotation_and_mirror_in_order() {
        let f = file(&[bx(b"ispe", &[0; 12]), bx(b"irot", &[1]), bx(b"imir", &[1])], &[1, 0x82, 3]);
        assert_eq!(
            transforms(&f, 10, 10),
            [Transform::Rotate { quarter_turns: 1 }, Transform::Mirror { vertical: false }]
        );
    }

    #[test]
    fn reads_centred_clean_aperture() {
        // 100x80 cropped to 60x40, centre moved 10 px right.
        let mut clap = Vec::new();
        for (n, d) in [(60u32, 1u32), (40, 1), (20, 2), (0, 1)] {
            clap.extend_from_slice(&n.to_be_bytes());
            clap.extend_from_slice(&d.to_be_bytes());
        }
        let f = file(&[bx(b"clap", &clap)], &[1]);
        assert_eq!(
            transforms(&f, 100, 80),
            [Transform::Crop { x: 30, y: 20, width: 60, height: 40 }]
        );
    }

    #[test]
    fn garbage_means_no_transforms() {
        assert!(transforms(b"not a box at all", 1, 1).is_empty());
        assert!(transforms(&[0, 0, 0, 200, b'm', b'e', b't', b'a'], 1, 1).is_empty());
    }
}
