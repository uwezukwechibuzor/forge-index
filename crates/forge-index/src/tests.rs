//! Tests for the ForgeIndex builder and runner.

use crate::builder::ForgeIndex;
use forge_index_config::{AddressConfig, ColumnType, ConfigBuilder, DatabaseConfig, SchemaBuilder};
use forge_index_core::abi::decoder::DecodedEvent;
use forge_index_core::types::Address;

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
    }
]"#;

fn test_schema() -> forge_index_config::Schema {
    SchemaBuilder::new()
        .table("transfers", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("from_addr", ColumnType::Address)
                .not_null()
                .column("to_addr", ColumnType::Address)
                .not_null()
                .column("amount", ColumnType::BigInt)
                .not_null()
        })
        .build()
}

fn test_config() -> forge_index_config::Config {
    ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = "https://eth.rpc.example".to_string();
        })
        .contract("ERC20", |c| {
            c.abi_json = ERC20_ABI.to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address =
                AddressConfig::Single(Address::from("0x0000000000000000000000000000000000000001"));
            c.start_block = 0;
        })
        .schema(test_schema())
        .database(DatabaseConfig::postgres("postgres://localhost/test"))
        .build()
        .unwrap()
}

async fn dummy_handler(_event: DecodedEvent, _ctx: serde_json::Value) -> Result<(), anyhow::Error> {
    Ok(())
}

#[test]
fn forge_index_without_config_returns_error() {
    let result = ForgeIndex::new()
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build();

    match result {
        Err(e) => assert!(
            e.to_string().contains("Config is required"),
            "expected config error, got: {}",
            e
        ),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn forge_index_without_schema_returns_error() {
    let result = ForgeIndex::new()
        .config(test_config())
        .on("ERC20:Transfer", dummy_handler)
        .build();

    match result {
        Err(e) => assert!(
            e.to_string().contains("Schema is required"),
            "expected schema error, got: {}",
            e
        ),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn forge_index_with_unknown_event_key_returns_error() {
    let result = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("UnknownContract:UnknownEvent", dummy_handler)
        .build();

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("UnknownContract:UnknownEvent"),
                "expected unknown handler error, got: {}",
                msg
            );
        }
        Ok(_) => panic!("expected error for unknown event key"),
    }
}

#[test]
fn forge_index_with_valid_config_builds_successfully() {
    let result = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build();

    assert!(result.is_ok(), "build should succeed");
}

#[test]
fn forge_index_warns_for_unhandled_events() {
    // Approval event exists in ABI but we only register Transfer handler.
    // This should succeed (just a warning, not an error).
    let result = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build();

    assert!(result.is_ok(), "should succeed even with unhandled events");
}

#[test]
fn forge_index_runner_has_correct_build_id() {
    let runner = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build()
        .unwrap();

    assert!(!runner.build_id().is_empty());
    assert_eq!(runner.build_id().len(), 64); // SHA-256 hex
}

#[test]
fn forge_index_build_id_is_deterministic() {
    let r1 = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build()
        .unwrap();

    let r2 = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build()
        .unwrap();

    assert_eq!(r1.build_id(), r2.build_id());
}

#[test]
fn forge_index_default_is_empty() {
    let fi = ForgeIndex::default();
    let result = fi.build();
    assert!(result.is_err());
}

// ── Deployment artifact validation ──────────────────────────────────

mod deployment_tests {
    use std::path::Path;

    fn workspace_root() -> &'static str {
        env!("CARGO_MANIFEST_DIR")
            .strip_suffix("/crates/forge-index")
            .unwrap_or(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn dockerfile_exists_and_has_required_stages() {
        let path = Path::new(workspace_root()).join("Dockerfile");
        assert!(path.exists(), "Dockerfile should exist at workspace root");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("FROM rust:"),
            "should have rust base image"
        );
        assert!(content.contains("cargo-chef"), "should use cargo-chef");
        assert!(content.contains("AS planner"), "should have planner stage");
        assert!(content.contains("AS builder"), "should have builder stage");
        assert!(content.contains("AS runtime"), "should have runtime stage");
        assert!(
            content.contains("debian:bookworm-slim"),
            "runtime should use debian slim"
        );
        assert!(
            content.contains("forge-index-cli"),
            "should build forge-index-cli"
        );
        assert!(content.contains("ENTRYPOINT"), "should have an ENTRYPOINT");
    }

    #[test]
    fn docker_compose_yml_is_valid_yaml_with_required_services() {
        let path = Path::new(workspace_root()).join("docker-compose.yml");
        assert!(path.exists(), "docker-compose.yml should exist");
        let content = std::fs::read_to_string(&path).unwrap();
        let yaml: serde_json::Value =
            serde_yaml::from_str(&content).expect("docker-compose.yml should be valid YAML");

        let services = yaml.get("services").expect("should have services key");
        assert!(
            services.get("postgres").is_some(),
            "should have postgres service"
        );
        assert!(
            services.get("indexer").is_some(),
            "should have indexer service"
        );
        assert!(
            services.get("prometheus").is_some(),
            "should have prometheus service"
        );
        assert!(
            services.get("grafana").is_some(),
            "should have grafana service"
        );
    }

    #[test]
    fn docker_compose_dev_yml_is_valid_yaml() {
        let path = Path::new(workspace_root()).join("docker-compose.dev.yml");
        assert!(path.exists(), "docker-compose.dev.yml should exist");
        let content = std::fs::read_to_string(&path).unwrap();
        let yaml: serde_json::Value =
            serde_yaml::from_str(&content).expect("docker-compose.dev.yml should be valid YAML");

        let services = yaml.get("services").expect("should have services key");
        assert!(services.get("postgres").is_some(), "should have postgres");
        assert!(services.get("indexer").is_some(), "should have indexer");
        // dev compose should NOT have prometheus/grafana
        assert!(
            services.get("prometheus").is_none(),
            "dev compose should not have prometheus"
        );
    }

    #[test]
    fn env_example_has_all_forge_variables() {
        let path = Path::new(workspace_root()).join(".env.example");
        assert!(path.exists(), ".env.example should exist");
        let content = std::fs::read_to_string(&path).unwrap();

        let required_vars = [
            "DATABASE_URL",
            "RPC_URL_1",
            "FORGE_ENV",
            "FORGE_PORT",
            "FORGE_SCHEMA",
            "FORGE_LOG_LEVEL",
            "FORGE_API_KEY",
            "FORGE_RPC_RATE_LIMIT",
            "POSTGRES_DB",
            "POSTGRES_USER",
            "POSTGRES_PASSWORD",
            "GF_SECURITY_ADMIN_PASSWORD",
        ];

        for var in &required_vars {
            assert!(
                content.contains(var),
                ".env.example should document {}",
                var
            );
        }
    }

    #[test]
    fn prometheus_yml_is_valid_yaml() {
        let path = Path::new(workspace_root()).join("monitoring/prometheus.yml");
        assert!(path.exists(), "prometheus.yml should exist");
        let content = std::fs::read_to_string(&path).unwrap();
        let yaml: serde_json::Value =
            serde_yaml::from_str(&content).expect("prometheus.yml should be valid YAML");

        let scrape = yaml
            .get("scrape_configs")
            .expect("should have scrape_configs");
        assert!(scrape.is_array(), "scrape_configs should be an array");
        let jobs = scrape.as_array().unwrap();
        assert!(
            jobs.iter()
                .any(|j| j.get("job_name").and_then(|v| v.as_str()) == Some("forge-index")),
            "should have forge-index job"
        );
    }

    #[test]
    fn grafana_dashboard_json_is_valid() {
        let path =
            Path::new(workspace_root()).join("monitoring/grafana/dashboards/forge-index.json");
        assert!(path.exists(), "forge-index.json dashboard should exist");
        let content = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&content).expect("dashboard should be valid JSON");

        assert_eq!(json["title"], "forge-index");
        assert_eq!(json["uid"], "forge-index-overview");

        let panels = json["panels"].as_array().expect("should have panels");
        assert!(
            panels.len() >= 10,
            "should have at least 10 panels, got {}",
            panels.len()
        );

        // Verify key panels exist
        let titles: Vec<&str> = panels.iter().filter_map(|p| p["title"].as_str()).collect();
        assert!(titles.iter().any(|t| t.contains("Events Indexed")));
        assert!(titles.iter().any(|t| t.contains("Blocks Processed")));
        assert!(titles.iter().any(|t| t.contains("Indexer Lag")));
        assert!(titles.iter().any(|t| t.contains("Backfill Progress")));
        assert!(titles.iter().any(|t| t.contains("RPC Request Duration")));
        assert!(titles.iter().any(|t| t.contains("DB Flush Duration")));
        assert!(titles.iter().any(|t| t.contains("Write Buffer")));
        assert!(titles.iter().any(|t| t.contains("HTTP Request Rate")));
        assert!(titles.iter().any(|t| t.contains("HTTP Error Rate")));
        assert!(titles.iter().any(|t| t.contains("RPC Error Rate")));
    }

    #[test]
    fn grafana_datasource_provisioning_is_valid() {
        let path = Path::new(workspace_root())
            .join("monitoring/grafana/provisioning/datasources/prometheus.yml");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let yaml: serde_json::Value =
            serde_yaml::from_str(&content).expect("datasource config should be valid YAML");
        assert!(yaml.get("datasources").is_some());
    }

    #[test]
    fn grafana_dashboard_provisioning_is_valid() {
        let path = Path::new(workspace_root())
            .join("monitoring/grafana/provisioning/dashboards/forge.yml");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let yaml: serde_json::Value =
            serde_yaml::from_str(&content).expect("dashboard provisioning should be valid YAML");
        assert!(yaml.get("providers").is_some());
    }

    #[test]
    fn dockerignore_exists_and_excludes_target() {
        let path = Path::new(workspace_root()).join(".dockerignore");
        assert!(path.exists(), ".dockerignore should exist");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("target/"));
        assert!(content.contains(".git/"));
    }

    #[test]
    fn makefile_has_all_required_targets() {
        let path = Path::new(workspace_root()).join("Makefile");
        assert!(path.exists(), "Makefile should exist");
        let content = std::fs::read_to_string(&path).unwrap();

        let targets = [
            "build:",
            "build-release:",
            "test:",
            "test-integration:",
            "fmt:",
            "lint:",
            "clean:",
            "docker-build:",
            "docker-push:",
            "dev:",
            "prod:",
            "down:",
            "logs:",
            "migrate:",
            "codegen:",
            "bench:",
        ];

        for target in &targets {
            assert!(
                content.contains(target),
                "Makefile should have target {}",
                target
            );
        }
    }
}
