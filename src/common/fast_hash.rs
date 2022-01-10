use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, Serialize)]
#[repr(transparent)]
pub struct FastHash(u64);

pub fn compute_fast_hash(bytes: &[u8]) -> FastHash {
    FastHash(fxhash::hash64(bytes))
}
