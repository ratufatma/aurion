use aurion_primitives::hash::Hash256;

pub fn compute_merkle_root(hashes: &[Hash256]) -> Hash256 {
    if hashes.is_empty() {
        return Hash256::ZERO;
    }

    let mut current_layer = hashes.to_vec();
    while current_layer.len() > 1 {
        let mut next_layer = Vec::with_capacity(current_layer.len().div_ceil(2));
        for chunk in current_layer.chunks(2) {
            let left = chunk[0];
            let right = if chunk.len() == 2 { chunk[1] } else { chunk[0] };
            let mut combined = [0u8; 64];
            combined[..32].copy_from_slice(left.as_bytes());
            combined[32..].copy_from_slice(right.as_bytes());
            next_layer.push(Hash256::digest(&combined));
        }
        current_layer = next_layer;
    }

    current_layer[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_root_determinism() {
        let h1 = Hash256::digest(b"tx1");
        let h2 = Hash256::digest(b"tx2");
        let root1 = compute_merkle_root(&[h1, h2]);
        let root2 = compute_merkle_root(&[h1, h2]);
        assert_eq!(root1, root2);
        assert_ne!(root1, Hash256::ZERO);
    }
}
