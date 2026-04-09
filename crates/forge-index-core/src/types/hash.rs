//! 32-byte hash newtype wrapping `[u8; 32]`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A 32-byte hash, typically a block hash or transaction hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Hash32(pub [u8; 32]);

impl Hash32 {
    /// Creates a `Hash32` from a hex string, optionally prefixed with `0x`.
    ///
    /// Returns `None` if the input is not valid hex or not exactly 32 bytes.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        if s.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s, &mut bytes).ok()?;
        Some(Self(bytes))
    }
}

impl From<&str> for Hash32 {
    fn from(s: &str) -> Self {
        Self::from_hex(s).expect("invalid hex hash")
    }
}

impl From<alloy::primitives::B256> for Hash32 {
    fn from(b: alloy::primitives::B256) -> Self {
        Self(b.0)
    }
}

impl From<Hash32> for alloy::primitives::B256 {
    fn from(h: Hash32) -> Self {
        alloy::primitives::B256::from(h.0)
    }
}

impl fmt::Display for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl Serialize for Hash32 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hash32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid hex hash"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash32_from_alloy_b256_roundtrips() {
        let b256 = alloy::primitives::B256::from([0xAB; 32]);
        let hash = Hash32::from(b256);
        let back: alloy::primitives::B256 = hash.into();
        assert_eq!(b256, back);
    }

    #[test]
    fn hash32_display_is_0x_prefixed_hex() {
        let hash = Hash32([0x00; 32]);
        assert_eq!(
            hash.to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn hash32_from_hex_roundtrips() {
        let hex_str = "0xabababababababababababababababababababababababababababababababab";
        let hash = Hash32::from(hex_str);
        let displayed = hash.to_string();
        let roundtripped = Hash32::from(displayed.as_str());
        assert_eq!(hash, roundtripped);
    }

    #[test]
    fn hash32_serde_roundtrip() {
        let hash = Hash32::from(alloy::primitives::B256::from([0xCD; 32]));
        let json = serde_json::to_string(&hash).unwrap();
        let deserialized: Hash32 = serde_json::from_str(&json).unwrap();
        assert_eq!(hash, deserialized);
    }
}
