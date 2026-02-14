#[cfg(test)]
mod tests {
    use gateway_runtime::gateway::Gateway;
    use gateway_runtime::layers::health::HealthCheckConfig;
    use gateway_runtime::router::Router;
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    use http_body_util::BodyExt;

    #[tokio::test]
    async fn test_gateway_health_check() {
        // Create a dummy router
        let router: Router<
            tower::util::BoxCloneService<
                gateway_runtime::GatewayRequest,
                gateway_runtime::GatewayResponse,
                gateway_runtime::GatewayError,
            >,
        > = Router::new();

        let config = HealthCheckConfig {
            liveness_path: "/healthz".to_string(),
            readiness_path: "/readyz".to_string(),
            ..Default::default()
        };

        let gateway = Gateway::new(router).with_health_check(config);

        let service = gateway.into_service();

        // Test Liveness
        let req = Request::builder().uri("/healthz").body(Vec::new()).unwrap();

        let resp = service.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Check body
        let body_bytes = resp.collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(body_str.contains("SERVING"));

        // Test Readiness
        let req = Request::builder().uri("/readyz").body(Vec::new()).unwrap();

        let resp = service.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
