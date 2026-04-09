//! ABI JSON parsing.

use crate::abi::types::{AbiError, AbiEvent, AbiFunction, AbiParam, AbiType};

/// The result of parsing a full ABI JSON array.
#[derive(Debug, Clone)]
pub struct ParsedAbi {
    /// All events in the ABI.
    pub events: Vec<AbiEvent>,
    /// All functions in the ABI.
    pub functions: Vec<AbiFunction>,
}

/// Parses a JSON string of a full ABI array into events and functions.
pub fn parse_abi(json: &str) -> Result<ParsedAbi, AbiError> {
    let items: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| AbiError::ParseError(e.to_string()))?;

    let mut events = Vec::new();
    let mut functions = Vec::new();

    for item in &items {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match item_type {
            "event" => {
                events.push(parse_event(item)?);
            }
            "function" => {
                functions.push(parse_function(item)?);
            }
            _ => {} // Skip constructors, fallback, receive, errors
        }
    }

    Ok(ParsedAbi { events, functions })
}

fn parse_event(item: &serde_json::Value) -> Result<AbiEvent, AbiError> {
    let name = item
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AbiError::ParseError("event missing name".to_string()))?
        .to_string();

    let inputs = parse_params(
        item.get("inputs")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new()),
    )?;

    let signature = AbiEvent::signature_string(&name, &inputs);
    let selector = AbiEvent::compute_selector(&signature);

    Ok(AbiEvent {
        name,
        signature,
        selector,
        inputs,
    })
}

fn parse_function(item: &serde_json::Value) -> Result<AbiFunction, AbiError> {
    let name = item
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AbiError::ParseError("function missing name".to_string()))?
        .to_string();

    let inputs = parse_params(
        item.get("inputs")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new()),
    )?;

    let outputs = parse_params(
        item.get("outputs")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new()),
    )?;

    let state_mutability = item
        .get("stateMutability")
        .and_then(|v| v.as_str())
        .unwrap_or("nonpayable")
        .to_string();

    Ok(AbiFunction {
        name,
        inputs,
        outputs,
        state_mutability,
    })
}

fn parse_params(arr: &[serde_json::Value]) -> Result<Vec<AbiParam>, AbiError> {
    arr.iter().map(parse_param).collect()
}

fn parse_param(item: &serde_json::Value) -> Result<AbiParam, AbiError> {
    let name = item
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let indexed = item
        .get("indexed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let type_str = item
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AbiError::ParseError("param missing type".to_string()))?;

    let abi_type = if type_str == "tuple" || type_str == "tuple[]" {
        let empty = Vec::new();
        let components = item
            .get("components")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty);
        let params = parse_params(components)?;
        if type_str == "tuple[]" {
            AbiType::Array(Box::new(AbiType::Tuple(params)))
        } else {
            AbiType::Tuple(params)
        }
    } else {
        parse_type_string(type_str)?
    };

    Ok(AbiParam {
        name,
        abi_type,
        indexed,
    })
}

/// Parses a Solidity type string (e.g. "uint256", "address", "bytes32[]").
pub fn parse_type_string(s: &str) -> Result<AbiType, AbiError> {
    // Handle arrays
    if let Some(inner) = s.strip_suffix("[]") {
        let elem = parse_type_string(inner)?;
        return Ok(AbiType::Array(Box::new(elem)));
    }

    // Handle fixed arrays like "uint256[3]"
    if let Some(bracket_pos) = s.rfind('[') {
        if s.ends_with(']') {
            let inner = &s[..bracket_pos];
            let size_str = &s[bracket_pos + 1..s.len() - 1];
            if let Ok(size) = size_str.parse::<usize>() {
                let elem = parse_type_string(inner)?;
                return Ok(AbiType::FixedArray(Box::new(elem), size));
            }
        }
    }

    match s {
        "address" => Ok(AbiType::Address),
        "bool" => Ok(AbiType::Bool),
        "string" => Ok(AbiType::String),
        "bytes" => Ok(AbiType::Bytes),
        _ if s.starts_with("uint") => {
            let bits: usize = s[4..].parse().unwrap_or(256);
            Ok(AbiType::Uint(bits))
        }
        _ if s.starts_with("int") => {
            let bits: usize = s[3..].parse().unwrap_or(256);
            Ok(AbiType::Int(bits))
        }
        _ if s.starts_with("bytes") => {
            let n: usize = s[5..]
                .parse()
                .map_err(|_| AbiError::UnknownType(s.to_string()))?;
            Ok(AbiType::FixedBytes(n))
        }
        _ => Err(AbiError::UnknownType(s.to_string())),
    }
}
