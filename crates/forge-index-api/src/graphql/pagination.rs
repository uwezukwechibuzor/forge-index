//! Cursor-based pagination for GraphQL list queries.

use serde::{Deserialize, Serialize};

use crate::error::ApiError;

/// Data encoded in a pagination cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorData {
    /// The primary key value at the cursor position.
    pub pk_value: String,
    /// The column used for ordering.
    pub order_col: String,
    /// The value of the order column at the cursor position.
    pub order_val: serde_json::Value,
}

/// Encodes cursor data into a base64 string.
pub fn encode_cursor(pk_value: &str, order_col: &str, order_val: &serde_json::Value) -> String {
    let data = CursorData {
        pk_value: pk_value.to_string(),
        order_col: order_col.to_string(),
        order_val: order_val.clone(),
    };
    let json = serde_json::to_string(&data).unwrap_or_default();
    base64_encode(&json)
}

/// Decodes a cursor string into cursor data.
pub fn decode_cursor(cursor: &str) -> Result<CursorData, ApiError> {
    let json = base64_decode(cursor)
        .map_err(|_| ApiError::BadRequest("Invalid cursor encoding".to_string()))?;
    serde_json::from_str(&json)
        .map_err(|_| ApiError::BadRequest("Invalid cursor format".to_string()))
}

fn base64_encode(input: &str) -> String {
    // Simple base64 encoding without external crate
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(input: &str) -> Result<String, ()> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let input = input.trim_end_matches('=');
    let mut bytes = Vec::new();
    let chars: Vec<u8> = input
        .bytes()
        .map(|b| {
            CHARS
                .iter()
                .position(|&c| c == b)
                .map(|p| p as u8)
                .unwrap_or(0)
        })
        .collect();
    for chunk in chars.chunks(4) {
        if chunk.len() < 2 {
            break;
        }
        let b0 = chunk[0] as u32;
        let b1 = chunk[1] as u32;
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let b3 = if chunk.len() > 3 { chunk[3] as u32 } else { 0 };
        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;
        bytes.push(((triple >> 16) & 0xFF) as u8);
        if chunk.len() > 2 {
            bytes.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk.len() > 3 {
            bytes.push((triple & 0xFF) as u8);
        }
    }
    String::from_utf8(bytes).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_roundtrip() {
        let cursor = encode_cursor("pk1", "created_at", &serde_json::json!(12345));
        let decoded = decode_cursor(&cursor).unwrap();
        assert_eq!(decoded.pk_value, "pk1");
        assert_eq!(decoded.order_col, "created_at");
        assert_eq!(decoded.order_val, serde_json::json!(12345));
    }
}
