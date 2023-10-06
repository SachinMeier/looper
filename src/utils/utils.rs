use bdk::bitcoin::hashes::{sha256, Hash};

pub fn sha256(input: &[u8]) -> [u8; 32] {
    return sha256::Hash::hash(input).into_inner().into();
}
