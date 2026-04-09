//! ABI log decoder.

use crate::abi::parser::ParsedAbi;
use crate::abi::types::{AbiError, AbiEvent, AbiParam, AbiType};
use crate::types::{Address, Hash32, Log};
use indexmap::IndexMap;
use std::collections::HashMap;

/// A decoded event parameter value.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedParam {
    /// Unsigned integer that fits in u128.
    Uint(u128),
    /// Unsigned 256-bit integer stored as decimal string (may exceed u128).
    Uint256(String),
    /// Signed integer that fits in i128.
    Int(i128),
    /// Signed 256-bit integer stored as decimal string.
    Int256(String),
    /// Ethereum address.
    Address(Address),
    /// Boolean.
    Bool(bool),
    /// Dynamic bytes.
    Bytes(Vec<u8>),
    /// Fixed-size bytes.
    FixedBytes(Vec<u8>),
    /// UTF-8 string.
    String_(String),
    /// Dynamic array of values.
    Array(Vec<DecodedParam>),
    /// Tuple of values.
    Tuple(Vec<DecodedParam>),
}

/// A fully decoded EVM event.
#[derive(Debug, Clone)]
pub struct DecodedEvent {
    /// The event name (e.g. "Transfer").
    pub name: String,
    /// The contract name.
    pub contract_name: String,
    /// Named parameters in order.
    pub params: IndexMap<String, DecodedParam>,
    /// The raw log entry.
    pub raw_log: Log,
}

impl DecodedEvent {
    /// Gets a parameter by name, returning an error if not found.
    pub fn get(&self, name: &str) -> Result<&DecodedParam, AbiError> {
        self.params
            .get(name)
            .ok_or_else(|| AbiError::DecodeMismatch(format!("parameter '{}' not found", name)))
    }
}

/// Decodes raw EVM logs into typed events using ABI definitions.
pub struct LogDecoder {
    events: HashMap<Hash32, AbiEvent>,
}

impl LogDecoder {
    /// Creates a new decoder from a parsed ABI.
    pub fn new(abi: &ParsedAbi) -> Self {
        let mut events = HashMap::new();
        for event in &abi.events {
            events.insert(event.selector, event.clone());
        }
        Self { events }
    }

    /// Decodes a raw log into a `DecodedEvent`.
    pub fn decode(&self, log: &Log, contract_name: &str) -> Result<DecodedEvent, AbiError> {
        if log.topics.is_empty() {
            return Err(AbiError::InvalidSelector);
        }

        let selector = &log.topics[0];
        let abi_event = self.events.get(selector).ok_or(AbiError::InvalidSelector)?;

        let mut params = IndexMap::new();
        let mut topic_idx = 1; // topic[0] is the selector
        let mut data_offset = 0;

        for input in &abi_event.inputs {
            if input.indexed {
                // Indexed parameters are in topics
                if topic_idx >= log.topics.len() {
                    return Err(AbiError::DecodeMismatch(format!(
                        "missing topic for indexed param '{}'",
                        input.name
                    )));
                }
                let topic_bytes = log.topics[topic_idx].0;
                let value = decode_indexed_param(&input.abi_type, &topic_bytes);
                params.insert(input.name.clone(), value);
                topic_idx += 1;
            } else {
                // Non-indexed parameters are in data
                let (value, consumed) =
                    decode_param_from_data(&input.abi_type, &log.data, data_offset)?;
                params.insert(input.name.clone(), value);
                data_offset += consumed;
            }
        }

        Ok(DecodedEvent {
            name: abi_event.name.clone(),
            contract_name: contract_name.to_string(),
            params,
            raw_log: log.clone(),
        })
    }
}

/// Decodes an indexed parameter from a 32-byte topic.
fn decode_indexed_param(abi_type: &AbiType, topic: &[u8; 32]) -> DecodedParam {
    match abi_type {
        AbiType::Address => {
            // Address is right-aligned in 32 bytes
            let mut addr = [0u8; 20];
            addr.copy_from_slice(&topic[12..32]);
            DecodedParam::Address(Address(addr))
        }
        AbiType::Bool => DecodedParam::Bool(topic[31] != 0),
        AbiType::Uint(bits) => decode_uint(topic, *bits),
        AbiType::Int(bits) => decode_int(topic, *bits),
        AbiType::FixedBytes(n) => DecodedParam::FixedBytes(topic[..(*n).min(32)].to_vec()),
        // Dynamic types (bytes, string, array) are stored as keccak256 hash
        // when indexed — cannot be decoded back, store as raw bytes
        _ => DecodedParam::FixedBytes(topic.to_vec()),
    }
}

/// Decodes a parameter from the data section.
fn decode_param_from_data(
    abi_type: &AbiType,
    data: &[u8],
    offset: usize,
) -> Result<(DecodedParam, usize), AbiError> {
    match abi_type {
        AbiType::Uint(bits) => {
            let slot = read_slot(data, offset)?;
            Ok((decode_uint(&slot, *bits), 32))
        }
        AbiType::Int(bits) => {
            let slot = read_slot(data, offset)?;
            Ok((decode_int(&slot, *bits), 32))
        }
        AbiType::Address => {
            let slot = read_slot(data, offset)?;
            let mut addr = [0u8; 20];
            addr.copy_from_slice(&slot[12..32]);
            Ok((DecodedParam::Address(Address(addr)), 32))
        }
        AbiType::Bool => {
            let slot = read_slot(data, offset)?;
            Ok((DecodedParam::Bool(slot[31] != 0), 32))
        }
        AbiType::FixedBytes(n) => {
            let slot = read_slot(data, offset)?;
            Ok((DecodedParam::FixedBytes(slot[..(*n).min(32)].to_vec()), 32))
        }
        AbiType::Bytes => {
            let (value, _) = decode_dynamic_bytes(data, offset)?;
            Ok((DecodedParam::Bytes(value), 32)) // head is 32 bytes (offset pointer)
        }
        AbiType::String => {
            let (value, _) = decode_dynamic_bytes(data, offset)?;
            let s = String::from_utf8_lossy(&value).to_string();
            Ok((DecodedParam::String_(s), 32))
        }
        AbiType::Array(inner) => {
            let (value, _) = decode_dynamic_array(inner, data, offset)?;
            Ok((DecodedParam::Array(value), 32))
        }
        AbiType::FixedArray(inner, len) => {
            if inner.is_dynamic() {
                // Fixed array of dynamic types has a head offset
                let (value, _) = decode_fixed_array_dynamic(inner, *len, data, offset)?;
                Ok((DecodedParam::Array(value), 32))
            } else {
                let mut items = Vec::with_capacity(*len);
                let mut pos = offset;
                for _ in 0..*len {
                    let (item, consumed) = decode_param_from_data(inner, data, pos)?;
                    items.push(item);
                    pos += consumed;
                }
                Ok((DecodedParam::Array(items), 32 * len))
            }
        }
        AbiType::Tuple(params) => {
            if abi_type.is_dynamic() {
                // Dynamic tuple: head contains offset
                let head_slot = read_slot(data, offset)?;
                let abs_offset = u256_to_usize(&head_slot)?;
                let (items, _) = decode_tuple_at(params, data, abs_offset)?;
                Ok((DecodedParam::Tuple(items), 32))
            } else {
                let (items, total) = decode_tuple_at(params, data, offset)?;
                Ok((DecodedParam::Tuple(items), total))
            }
        }
    }
}

fn decode_tuple_at(
    params: &[AbiParam],
    data: &[u8],
    base: usize,
) -> Result<(Vec<DecodedParam>, usize), AbiError> {
    let mut items = Vec::with_capacity(params.len());
    let mut pos = base;
    for param in params {
        if param.abi_type.is_dynamic() {
            // Head contains relative offset from base
            let head_slot = read_slot(data, pos)?;
            let rel_offset = u256_to_usize(&head_slot)?;
            let abs_offset = base + rel_offset;
            let (value, _) = decode_param_from_data_at_abs(&param.abi_type, data, abs_offset)?;
            items.push(value);
            pos += 32;
        } else {
            let (value, consumed) = decode_param_from_data(&param.abi_type, data, pos)?;
            items.push(value);
            pos += consumed;
        }
    }
    Ok((items, pos - base))
}

/// Decode a param at an absolute position for dynamic types inside tuples.
fn decode_param_from_data_at_abs(
    abi_type: &AbiType,
    data: &[u8],
    abs_offset: usize,
) -> Result<(DecodedParam, usize), AbiError> {
    match abi_type {
        AbiType::Bytes => {
            let len_slot = read_slot(data, abs_offset)?;
            let len = u256_to_usize(&len_slot)?;
            let start = abs_offset + 32;
            let end = (start + len).min(data.len());
            Ok((
                DecodedParam::Bytes(data[start..end].to_vec()),
                32 + padded_len(len),
            ))
        }
        AbiType::String => {
            let len_slot = read_slot(data, abs_offset)?;
            let len = u256_to_usize(&len_slot)?;
            let start = abs_offset + 32;
            let end = (start + len).min(data.len());
            let s = String::from_utf8_lossy(&data[start..end]).to_string();
            Ok((DecodedParam::String_(s), 32 + padded_len(len)))
        }
        AbiType::Array(inner) => {
            let len_slot = read_slot(data, abs_offset)?;
            let len = u256_to_usize(&len_slot)?;
            let mut items = Vec::with_capacity(len);
            let mut pos = abs_offset + 32;
            for _ in 0..len {
                let (item, consumed) = decode_param_from_data(inner, data, pos)?;
                items.push(item);
                pos += consumed;
            }
            Ok((DecodedParam::Array(items), pos - abs_offset))
        }
        _ => decode_param_from_data(abi_type, data, abs_offset),
    }
}

fn decode_dynamic_bytes(data: &[u8], head_offset: usize) -> Result<(Vec<u8>, usize), AbiError> {
    // Head contains the offset to the start of the dynamic data
    let head_slot = read_slot(data, head_offset)?;
    let abs_offset = u256_to_usize(&head_slot)?;

    let len_slot = read_slot(data, abs_offset)?;
    let len = u256_to_usize(&len_slot)?;

    let start = abs_offset + 32;
    let end = (start + len).min(data.len());
    Ok((data[start..end].to_vec(), 32 + padded_len(len)))
}

fn decode_dynamic_array(
    inner: &AbiType,
    data: &[u8],
    head_offset: usize,
) -> Result<(Vec<DecodedParam>, usize), AbiError> {
    let head_slot = read_slot(data, head_offset)?;
    let abs_offset = u256_to_usize(&head_slot)?;

    let len_slot = read_slot(data, abs_offset)?;
    let len = u256_to_usize(&len_slot)?;

    let mut items = Vec::with_capacity(len);
    let mut pos = abs_offset + 32;
    for _ in 0..len {
        let (item, consumed) = decode_param_from_data(inner, data, pos)?;
        items.push(item);
        pos += consumed;
    }

    Ok((items, pos - abs_offset))
}

fn decode_fixed_array_dynamic(
    inner: &AbiType,
    len: usize,
    data: &[u8],
    head_offset: usize,
) -> Result<(Vec<DecodedParam>, usize), AbiError> {
    let head_slot = read_slot(data, head_offset)?;
    let abs_offset = u256_to_usize(&head_slot)?;

    let mut items = Vec::with_capacity(len);
    let mut pos = abs_offset;
    for _ in 0..len {
        let (item, consumed) = decode_param_from_data(inner, data, pos)?;
        items.push(item);
        pos += consumed;
    }

    Ok((items, pos - abs_offset))
}

fn decode_uint(slot: &[u8; 32], bits: usize) -> DecodedParam {
    if bits <= 128 {
        // Fits in u128
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&slot[16..32]);
        DecodedParam::Uint(u128::from_be_bytes(bytes))
    } else {
        // Use string representation for > 128 bits
        let value = u256_from_be_bytes(slot);
        DecodedParam::Uint256(value)
    }
}

fn decode_int(slot: &[u8; 32], bits: usize) -> DecodedParam {
    if bits <= 128 {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&slot[16..32]);
        let raw = u128::from_be_bytes(bytes);
        // Sign-extend: if the sign bit of the original width is set, interpret as negative
        let sign_bit = 1u128 << (bits - 1);
        let val = if raw & sign_bit != 0 {
            // Negative: sign extend to i128
            let mask = !0u128 << bits;
            (raw | mask) as i128
        } else {
            raw as i128
        };
        DecodedParam::Int(val)
    } else {
        // Full 256-bit: store as string
        let is_negative = slot[0] & 0x80 != 0;
        if is_negative {
            // Two's complement for 256-bit
            DecodedParam::Int256(format!("-{}", twos_complement_256_str(slot)))
        } else {
            DecodedParam::Int256(u256_from_be_bytes(slot))
        }
    }
}

fn read_slot(data: &[u8], offset: usize) -> Result<[u8; 32], AbiError> {
    if offset + 32 > data.len() {
        return Err(AbiError::DecodeMismatch(format!(
            "data too short: need {} bytes at offset {}, have {}",
            32,
            offset,
            data.len()
        )));
    }
    let mut slot = [0u8; 32];
    slot.copy_from_slice(&data[offset..offset + 32]);
    Ok(slot)
}

fn u256_to_usize(slot: &[u8; 32]) -> Result<usize, AbiError> {
    // Check that the value fits in usize
    for &b in &slot[..24] {
        if b != 0 {
            return Err(AbiError::DecodeMismatch("offset too large".to_string()));
        }
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&slot[24..32]);
    Ok(u64::from_be_bytes(bytes) as usize)
}

fn padded_len(len: usize) -> usize {
    len.div_ceil(32) * 32
}

/// Converts 32 big-endian bytes to a decimal string.
fn u256_from_be_bytes(bytes: &[u8; 32]) -> String {
    // Use alloy's U256 for the conversion
    let u256 = alloy::primitives::U256::from_be_bytes(*bytes);
    u256.to_string()
}

/// Two's complement of a 256-bit negative number as a positive decimal string.
fn twos_complement_256_str(bytes: &[u8; 32]) -> String {
    let u256 = alloy::primitives::U256::from_be_bytes(*bytes);
    let complement = !u256 + alloy::primitives::U256::from(1u64);
    complement.to_string()
}
