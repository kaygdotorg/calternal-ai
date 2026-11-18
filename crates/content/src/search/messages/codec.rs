//! Elias–Fano encoding of increasing offsets in a 16,384-position block.
use super::WIDTH;

pub(super) fn encode(hits: &[u16]) -> Vec<u8> {
    let n = hits.len();
    if n <= 2 {
        let mut bytes = Vec::with_capacity(2 + 2 * n);
        bytes.extend_from_slice(&(n as u16).to_le_bytes());
        for x in hits {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
        return bytes;
    }
    let bits = (WIDTH / n).ilog2() as usize;
    let low = (n * bits).div_ceil(8);
    let high = ((WIDTH >> bits) + n).div_ceil(8);
    let mut out = vec![0u8; 2 + low + high];
    out[..2].copy_from_slice(&(n as u16).to_le_bytes());
    let mask = (1 << bits) - 1;
    for (i, &x) in hits.iter().enumerate() {
        let x = x as usize;
        let pos = i * bits;
        let piece = ((x & mask) << (pos % 8)) as u32;
        for (j, byte) in piece.to_le_bytes().into_iter().enumerate() {
            if pos / 8 + j >= low {
                break;
            }
            out[2 + pos / 8 + j] |= byte;
        }
        let top = (x >> bits) + i;
        out[2 + low + top / 8] |= 1 << (top % 8);
    }
    out
}

/// Decode the locally generated block. Invalid rows are rejected before use.
pub(super) fn decode(data: &[u8]) -> Option<Vec<u16>> {
    let n = u16::from_le_bytes(data.get(..2)?.try_into().ok()?) as usize;
    if n > WIDTH {
        return None;
    }
    if n <= 2 {
        if data.len() != 2 + 2 * n {
            return None;
        }
        let positions: Vec<_> = data[2..]
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        return positions
            .iter()
            .all(|&p| usize::from(p) < WIDTH)
            .then_some(positions);
    }
    let bits = (WIDTH / n).ilog2() as usize;
    let low = (n * bits).div_ceil(8);
    let high = ((WIDTH >> bits) + n).div_ceil(8);
    if data.len() != 2 + low + high {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for (pos, &byte) in data[2 + low..].iter().enumerate() {
        let mut b = byte;
        while b != 0 {
            let i = out.len();
            let hi = (pos * 8 + b.trailing_zeros() as usize).checked_sub(i)?;
            let p = i * bits;
            let tail = [0, 1, 2].into_iter().fold(0usize, |sum, j| {
                sum + ((data.get(2 + p / 8 + j).copied().unwrap_or(0) as usize) << (j * 8))
            });
            let value = (hi << bits) | ((tail >> (p % 8)) & ((1 << bits) - 1));
            if value >= WIDTH || out.last().is_some_and(|&prior| usize::from(prior) >= value) {
                return None;
            }
            out.push(value as u16);
            b &= b - 1;
        }
    }
    (out.len() == n).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_and_dense_blocks_round_trip() {
        for stride in [1, 2, 17, 8192, WIDTH] {
            let values: Vec<_> = (0..WIDTH).step_by(stride).map(|i| i as u16).collect();
            let encoded = encode(&values);
            assert_eq!(decode(&encoded), Some(values));
            assert!(decode(&encoded[..encoded.len() - 1]).is_none());
        }
    }
}
