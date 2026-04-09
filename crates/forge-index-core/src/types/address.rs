//! EVM address newtype wrapping `[u8; 20]`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A 20-byte Ethereum address.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Address(pub [u8; 20]);

impl Address {
    /// Creates an `Address` from a hex string, optionally prefixed with `0x`.
    ///
    /// Returns `None` if the input is not valid hex or not exactly 20 bytes.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        if s.len() != 40 {
            return None;
        }
        let mut bytes = [0u8; 20];
        hex::decode_to_slice(s, &mut bytes).ok()?;
        Some(Self(bytes))
    }

    /// Returns the checksummed hex representation (EIP-55).
    fn checksum_hex(&self) -> String {
        let hex_lower = hex::encode(self.0);
        let hash = alloy::primitives::keccak256(hex_lower.as_bytes());
        let hash_hex = hex::encode(hash);

        let mut result = String::with_capacity(42);
        result.push_str("0x");
        for (i, c) in hex_lower.chars().enumerate() {
            if c.is_ascii_alphabetic() {
                let nibble = u8::from_str_radix(&hash_hex[i..i + 1], 16).unwrap_or(0);
                if nibble >= 8 {
                    result.push(c.to_ascii_uppercase());
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}

impl From<&str> for Address {
    fn from(s: &str) -> Self {
        Self::from_hex(s).expect("invalid hex address")
    }
}

impl From<alloy::primitives::Address> for Address {
    fn from(addr: alloy::primitives::Address) -> Self {
        Self(addr.0 .0)
    }
}

impl From<Address> for alloy::primitives::Address {
    fn from(addr: Address) -> Self {
        alloy::primitives::Address::from(addr.0)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.checksum_hex())
    }
}

impl Serialize for Address {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid hex address"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_from_hex_roundtrips_through_display() {
        let hex = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        let addr = Address::from(hex);
        let displayed = addr.to_string();
        let roundtripped = Address::from(displayed.as_str());
        assert_eq!(addr, roundtripped);
    }

    #[test]
    fn address_from_lowercase_hex_roundtrips() {
        let hex = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";
        let addr = Address::from(hex);
        let displayed = addr.to_string();
        let roundtripped = Address::from(displayed.as_str());
        assert_eq!(addr, roundtripped);
    }

    #[test]
    fn address_from_invalid_hex_returns_none() {
        assert!(Address::from_hex("0xZZZZ").is_none());
        assert!(Address::from_hex("0x1234").is_none());
        assert!(Address::from_hex("not_hex_at_all").is_none());
    }

    #[test]
    fn address_from_alloy_roundtrips() {
        let alloy_addr = alloy::primitives::Address::ZERO;
        let addr = Address::from(alloy_addr);
        let back: alloy::primitives::Address = addr.into();
        assert_eq!(alloy_addr, back);
    }

    #[test]
    fn address_serde_roundtrip() {
        let addr = Address::from("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let json = serde_json::to_string(&addr).unwrap();
        let deserialized: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(addr, deserialized);
    }
}
