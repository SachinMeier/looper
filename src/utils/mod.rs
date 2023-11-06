use bdk::bitcoin::hashes::{sha256, Hash};
use rand::Rng;

pub fn sha256(input: &[u8]) -> [u8; 32] {
    sha256::Hash::hash(input).to_byte_array()
}

pub fn rand_32_bytes() -> [u8; 32] {
    let mut rng = rand::thread_rng();

    let mut bytes: [u8; 32] = [0; 32];
    for b in &mut bytes {
        *b = rng.gen::<u8>();
    }

    bytes
}
