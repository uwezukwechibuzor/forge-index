//! Tests for ABI parsing, decoding, and event handling.

#[cfg(test)]
mod tests {
    use crate::abi::decoder::{DecodedEvent, DecodedParam, LogDecoder};
    use crate::abi::parser::parse_abi;
    use crate::abi::types::{AbiError, AbiEvent};
    use crate::registry::event_registry::EventRegistry;
    use crate::registry::handler::HandlerFn;
    use crate::types::{Address, Hash32, Log};

    const ERC20_ABI: &str = r#"[
        {
            "type": "event",
            "name": "Transfer",
            "inputs": [
                {"name": "from", "type": "address", "indexed": true},
                {"name": "to", "type": "address", "indexed": true},
                {"name": "value", "type": "uint256", "indexed": false}
            ]
        },
        {
            "type": "event",
            "name": "Approval",
            "inputs": [
                {"name": "owner", "type": "address", "indexed": true},
                {"name": "spender", "type": "address", "indexed": true},
                {"name": "value", "type": "uint256", "indexed": false}
            ]
        },
        {
            "type": "function",
            "name": "transfer",
            "inputs": [
                {"name": "to", "type": "address"},
                {"name": "value", "type": "uint256"}
            ],
            "outputs": [
                {"name": "", "type": "bool"}
            ],
            "stateMutability": "nonpayable"
        }
    ]"#;

    #[test]
    fn parse_abi_with_erc20_returns_correct_events() {
        let abi = parse_abi(ERC20_ABI).unwrap();
        assert_eq!(abi.events.len(), 2);
        assert_eq!(abi.functions.len(), 1);

        let transfer = &abi.events[0];
        assert_eq!(transfer.name, "Transfer");
        assert_eq!(transfer.inputs.len(), 3);
        assert!(transfer.inputs[0].indexed); // from
        assert!(transfer.inputs[1].indexed); // to
        assert!(!transfer.inputs[2].indexed); // value

        let approval = &abi.events[1];
        assert_eq!(approval.name, "Approval");
    }

    #[test]
    fn abi_event_selector_correct_for_transfer() {
        let abi = parse_abi(ERC20_ABI).unwrap();
        let transfer = &abi.events[0];

        // Transfer(address,address,uint256) keccak256
        let expected = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
        assert_eq!(transfer.selector.to_string(), expected);
        assert_eq!(transfer.signature, "Transfer(address,address,uint256)");
    }

    #[test]
    fn log_decoder_decode_transfer_log() {
        let abi = parse_abi(ERC20_ABI).unwrap();
        let decoder = LogDecoder::new(&abi);

        let selector =
            Hash32::from("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

        // from address in topic (padded to 32 bytes)
        let mut from_topic = [0u8; 32];
        from_topic[12..32].copy_from_slice(&[0xAA; 20]);

        // to address in topic
        let mut to_topic = [0u8; 32];
        to_topic[12..32].copy_from_slice(&[0xBB; 20]);

        // value = 1000 in data (32 bytes, big-endian)
        let mut data = [0u8; 32];
        data[31] = 0xE8; // 1000 = 0x3E8
        data[30] = 0x03;

        let log = Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0xCC; 20]),
            topics: vec![selector, Hash32(from_topic), Hash32(to_topic)],
            data: data.to_vec(),
            block_number: 100,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        };

        let decoded = decoder.decode(&log, "ERC20").unwrap();
        assert_eq!(decoded.name, "Transfer");
        assert_eq!(decoded.contract_name, "ERC20");

        // Check from address
        match decoded.get("from").unwrap() {
            DecodedParam::Address(addr) => {
                assert_eq!(addr.0, [0xAA; 20]);
            }
            other => panic!("expected Address, got {:?}", other),
        }

        // Check to address
        match decoded.get("to").unwrap() {
            DecodedParam::Address(addr) => {
                assert_eq!(addr.0, [0xBB; 20]);
            }
            other => panic!("expected Address, got {:?}", other),
        }

        // Check value
        match decoded.get("value").unwrap() {
            DecodedParam::Uint256(s) => {
                assert_eq!(s, "1000");
            }
            other => panic!("expected Uint256, got {:?}", other),
        }
    }

    #[test]
    fn log_decoder_unknown_selector_returns_error() {
        let abi = parse_abi(ERC20_ABI).unwrap();
        let decoder = LogDecoder::new(&abi);

        let log = Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![Hash32([0xFF; 32])], // Unknown selector
            data: vec![],
            block_number: 0,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        };

        let result = decoder.decode(&log, "ERC20");
        assert!(result.is_err());
        match result.unwrap_err() {
            AbiError::InvalidSelector => {}
            other => panic!("expected InvalidSelector, got {:?}", other),
        }
    }

    #[test]
    fn decoded_event_get_returns_param_by_name() {
        let abi = parse_abi(ERC20_ABI).unwrap();
        let decoder = LogDecoder::new(&abi);

        let selector =
            Hash32::from("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
        let log = Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![selector, Hash32([0; 32]), Hash32([0; 32])],
            data: vec![0; 32],
            block_number: 0,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        };

        let decoded = decoder.decode(&log, "Test").unwrap();
        assert!(decoded.get("from").is_ok());
        assert!(decoded.get("nonexistent").is_err());
    }

    #[test]
    fn decode_uint256_larger_than_u128_max() {
        let abi_json = r#"[{
            "type": "event",
            "name": "BigValue",
            "inputs": [{"name": "val", "type": "uint256", "indexed": false}]
        }]"#;
        let abi = parse_abi(abi_json).unwrap();
        let decoder = LogDecoder::new(&abi);

        let selector = abi.events[0].selector;

        // Create a value larger than u128::MAX (set high bytes)
        let mut data = [0u8; 32];
        data[0] = 0xFF; // This makes it > u128::MAX
        data[15] = 0xFF;
        data[31] = 0x01;

        let log = Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![selector],
            data: data.to_vec(),
            block_number: 0,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        };

        let decoded = decoder.decode(&log, "Test").unwrap();
        match decoded.get("val").unwrap() {
            DecodedParam::Uint256(s) => {
                // Should be a valid decimal string
                assert!(!s.is_empty());
                // Verify it's larger than u128::MAX
                assert!(s.len() > 38, "value should be very large, got: {}", s);
            }
            other => panic!("expected Uint256, got {:?}", other),
        }
    }

    #[test]
    fn decode_indexed_address_from_topic() {
        let abi_json = r#"[{
            "type": "event",
            "name": "Sender",
            "inputs": [{"name": "who", "type": "address", "indexed": true}]
        }]"#;
        let abi = parse_abi(abi_json).unwrap();
        let decoder = LogDecoder::new(&abi);

        let selector = abi.events[0].selector;
        let mut addr_topic = [0u8; 32];
        addr_topic[12..32].copy_from_slice(&[0x42; 20]);

        let log = Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![selector, Hash32(addr_topic)],
            data: vec![],
            block_number: 0,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        };

        let decoded = decoder.decode(&log, "Test").unwrap();
        match decoded.get("who").unwrap() {
            DecodedParam::Address(addr) => {
                assert_eq!(addr.0, [0x42; 20]);
            }
            other => panic!("expected Address, got {:?}", other),
        }
    }

    #[test]
    fn decode_dynamic_bytes_from_data() {
        let abi_json = r#"[{
            "type": "event",
            "name": "DataEvent",
            "inputs": [{"name": "payload", "type": "bytes", "indexed": false}]
        }]"#;
        let abi = parse_abi(abi_json).unwrap();
        let decoder = LogDecoder::new(&abi);

        let selector = abi.events[0].selector;

        // ABI-encoded bytes: offset (32) + length (3) + data (0xAABBCC padded to 32)
        let mut data = vec![0u8; 96];
        // Offset: 0x20 (32)
        data[31] = 0x20;
        // Length: 3
        data[63] = 0x03;
        // Data: 0xAABBCC
        data[64] = 0xAA;
        data[65] = 0xBB;
        data[66] = 0xCC;

        let log = Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![selector],
            data,
            block_number: 0,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        };

        let decoded = decoder.decode(&log, "Test").unwrap();
        match decoded.get("payload").unwrap() {
            DecodedParam::Bytes(b) => {
                assert_eq!(b, &[0xAA, 0xBB, 0xCC]);
            }
            other => panic!("expected Bytes, got {:?}", other),
        }
    }

    #[test]
    fn decode_tuple_type() {
        let abi_json = r#"[{
            "type": "event",
            "name": "Pair",
            "inputs": [{
                "name": "pair",
                "type": "tuple",
                "indexed": false,
                "components": [
                    {"name": "a", "type": "uint256"},
                    {"name": "b", "type": "uint256"}
                ]
            }]
        }]"#;
        let abi = parse_abi(abi_json).unwrap();
        let decoder = LogDecoder::new(&abi);
        let selector = abi.events[0].selector;

        // Tuple (a=100, b=200) — static tuple, no offset
        let mut data = vec![0u8; 64];
        data[31] = 100; // a = 100
        data[63] = 200; // b = 200

        let log = Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![selector],
            data,
            block_number: 0,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        };

        let decoded = decoder.decode(&log, "Test").unwrap();
        match decoded.get("pair").unwrap() {
            DecodedParam::Tuple(items) => {
                assert_eq!(items.len(), 2);
                match &items[0] {
                    DecodedParam::Uint256(s) => assert_eq!(s, "100"),
                    other => panic!("expected Uint256, got {:?}", other),
                }
                match &items[1] {
                    DecodedParam::Uint256(s) => assert_eq!(s, "200"),
                    other => panic!("expected Uint256, got {:?}", other),
                }
            }
            other => panic!("expected Tuple, got {:?}", other),
        }
    }

    #[test]
    fn event_registry_register_and_get_roundtrip() {
        let mut registry = EventRegistry::new();

        async fn handler(
            _event: DecodedEvent,
            _ctx: serde_json::Value,
        ) -> Result<(), anyhow::Error> {
            Ok(())
        }

        registry.register("ERC20:Transfer", handler);

        assert!(registry.has_handler("ERC20:Transfer"));
        assert!(!registry.has_handler("ERC20:Approval"));
        assert!(registry.get("ERC20:Transfer").is_some());
        assert!(registry.get("ERC20:Approval").is_none());

        let keys = registry.all_keys();
        assert_eq!(keys, vec!["ERC20:Transfer".to_string()]);
    }

    #[tokio::test]
    async fn handler_fn_blanket_impl_works() {
        async fn my_handler(
            event: DecodedEvent,
            _ctx: serde_json::Value,
        ) -> Result<(), anyhow::Error> {
            assert_eq!(event.name, "Transfer");
            Ok(())
        }

        let handler: Box<dyn HandlerFn> = Box::new(my_handler);

        // Create a minimal decoded event
        let event = DecodedEvent {
            name: "Transfer".to_string(),
            contract_name: "ERC20".to_string(),
            params: indexmap::IndexMap::new(),
            raw_log: Log {
                id: "test-0".to_string(),
                chain_id: 1,
                address: Address([0; 20]),
                topics: vec![],
                data: vec![],
                block_number: 0,
                block_hash: Hash32([0; 32]),
                transaction_hash: Hash32([0; 32]),
                log_index: 0,
                transaction_index: 0,
                removed: false,
            },
        };

        let result = handler.call(event, serde_json::Value::Null).await;
        assert!(result.is_ok());
    }
}
