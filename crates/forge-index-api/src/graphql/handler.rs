//! Axum handlers for GraphQL endpoints.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::Json;

use crate::graphql::schema_gen::GraphqlSchema;

/// Shared state for GraphQL handlers.
#[derive(Clone)]
pub struct GraphqlState {
    /// The dynamically generated GraphQL schema.
    pub schema: std::sync::Arc<GraphqlSchema>,
}

/// POST /graphql — executes a GraphQL query.
pub async fn graphql_handler(
    State(state): State<GraphqlState>,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let query = request.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let variables = request
        .get("variables")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let operation_name = request.get("operationName").and_then(|v| v.as_str());

    let mut gql_request = async_graphql::Request::new(query);

    if let serde_json::Value::Object(vars) = variables {
        let gql_vars: async_graphql::Variables =
            async_graphql::Variables::from_json(serde_json::Value::Object(vars));
        gql_request = gql_request.variables(gql_vars);
    }

    if let Some(op) = operation_name {
        gql_request = gql_request.operation_name(op);
    }

    let response = state.schema.schema.execute(gql_request).await;
    let json = serde_json::to_value(&response).unwrap_or_default();

    if response.errors.is_empty() {
        (StatusCode::OK, Json(json))
    } else {
        (StatusCode::BAD_REQUEST, Json(json))
    }
}

/// GET /graphql — serves the GraphQL Playground HTML.
pub async fn graphql_playground() -> impl IntoResponse {
    Html(PLAYGROUND_HTML)
}

/// The HTML for the GraphQL Playground.
pub const PLAYGROUND_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
  <title>GraphQL Playground</title>
  <link rel="stylesheet" href="https://unpkg.com/graphql-playground-react/build/static/css/index.css" />
  <script src="https://unpkg.com/graphql-playground-react/build/static/js/middleware.js"></script>
</head>
<body>
  <div id="root"></div>
  <script>
    window.addEventListener('load', function() {
      GraphQLPlayground.init(document.getElementById('root'), { endpoint: '/graphql' });
    });
  </script>
</body>
</html>"#;
