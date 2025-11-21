use crate::{start_mock_server, Matcher};
use raven::server::database::influx_client::InfluxClient;
use raven::server::database::InfluxConfig;
use serde_json::json;

#[tokio::test]
async fn test_bucket_provision_creates_bucket_when_missing() {
    let mut server = match start_mock_server("test_bucket_provision_creates_bucket_when_missing") {
        Some(server) => server,
        None => return,
    };
    let base_url = server.url();

    let health_mock = server
        .mock("GET", "/api/v2/health")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("{}")
        .expect(1)
        .create_async()
        .await;

    let list_mock = server
        .mock("GET", "/api/v2/buckets")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("name".into(), "test_bucket".into()),
            Matcher::UrlEncoded("org".into(), "test_org".into()),
            Matcher::UrlEncoded("limit".into(), "1".into()),
        ]))
        .match_header("Authorization", "Token test_token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({"buckets": []}).to_string())
        .expect(1)
        .create_async()
        .await;

    let org_mock = server
        .mock("GET", "/api/v2/orgs")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("org".into(), "test_org".into()),
            Matcher::UrlEncoded("limit".into(), "1".into()),
        ]))
        .match_header("Authorization", "Token test_token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({ "orgs": [{"id": "org_id_123", "name": "test_org"}] }).to_string())
        .expect(1)
        .create_async()
        .await;

    let create_mock = server
        .mock("POST", "/api/v2/buckets")
        .match_header("Authorization", "Token test_token")
        .match_header("content-type", "application/json")
        .match_body(Matcher::PartialJson(json!({
            "orgID": "org_id_123",
            "name": "test_bucket"
        })))
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body("{}")
        .expect(1)
        .create_async()
        .await;

    let client = InfluxClient::new(InfluxConfig {
        url: base_url,
        bucket: "test_bucket".into(),
        org: "test_org".into(),
        token: Some("test_token".into()),
        pool_size: 1,
        ..InfluxConfig::default()
    });

    client.connect().await.unwrap();

    health_mock.assert();
    list_mock.assert();
    org_mock.assert();
    create_mock.assert();
}

#[tokio::test]
async fn test_bucket_provision_noop_when_bucket_exists() {
    let mut server = match start_mock_server("test_bucket_provision_noop_when_bucket_exists") {
        Some(server) => server,
        None => return,
    };
    let base_url = server.url();

    let health_mock = server
        .mock("GET", "/api/v2/health")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("{}")
        .expect(1)
        .create_async()
        .await;

    let list_mock = server
        .mock("GET", "/api/v2/buckets")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("name".into(), "existing_bucket".into()),
            Matcher::UrlEncoded("org".into(), "test_org".into()),
            Matcher::UrlEncoded("limit".into(), "1".into()),
        ]))
        .match_header("Authorization", "Token test_token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({ "buckets": [{"name": "existing_bucket"}] }).to_string())
        .expect(1)
        .create_async()
        .await;

    let create_mock = server
        .mock("POST", "/api/v2/buckets")
        .expect(0)
        .create_async()
        .await;

    let client = InfluxClient::new(InfluxConfig {
        url: base_url,
        bucket: "existing_bucket".into(),
        org: "test_org".into(),
        token: Some("test_token".into()),
        pool_size: 1,
        ..InfluxConfig::default()
    });

    client.connect().await.unwrap();

    health_mock.assert();
    list_mock.assert();
    create_mock.assert();
}

#[tokio::test]
async fn test_bucket_provision_unauthorized_error() {
    let mut server = match start_mock_server("test_bucket_provision_unauthorized_error") {
        Some(server) => server,
        None => return,
    };
    let base_url = server.url();

    let health_mock = server
        .mock("GET", "/api/v2/health")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("{}")
        .expect(1)
        .create_async()
        .await;

    let list_mock = server
        .mock("GET", "/api/v2/buckets")
        .match_header("Authorization", "Token bad_token")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(json!({"code": "unauthorized", "message": "unauthorized"}).to_string())
        .expect(1)
        .create_async()
        .await;

    let client = InfluxClient::new(InfluxConfig {
        url: base_url,
        bucket: "test_bucket".into(),
        org: "test_org".into(),
        token: Some("bad_token".into()),
        pool_size: 1,
        ..InfluxConfig::default()
    });

    let result = client.connect().await;

    assert!(result.is_err());
    health_mock.assert();
    list_mock.assert();
}
