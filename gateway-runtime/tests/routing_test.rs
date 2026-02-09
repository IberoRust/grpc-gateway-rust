use gateway_internal::path_template::{Op, OpCode, Pattern};
use gateway_runtime::router::{route, Router};
use http::Method;

#[test]
fn test_routing_simple_match() {
    let mut router: Router<&str> = Router::new();
    // Pattern: /v1/echo
    // Corresponds to ops: LitPush(v1), LitPush(echo)
    let pattern = Pattern {
        ops: vec![
            Op {
                code: OpCode::LitPush,
                operand: 0,
            },
            Op {
                code: OpCode::LitPush,
                operand: 1,
            },
        ],
        pool: vec!["v1".to_string(), "echo".to_string()],
        vars: vec![],
        stack_size: 2,
        tail_len: 0,
        verb: None,
    };

    route(&mut router, Method::GET, pattern, "echo_handler");

    let result = router.match_request(&Method::GET, "/v1/echo");
    assert!(result.is_some());
    let (handler, params, _) = result.unwrap();
    assert_eq!(*handler, "echo_handler");
    assert!(params.is_empty());

    assert!(router.match_request(&Method::POST, "/v1/echo").is_none());
    assert!(router.match_request(&Method::GET, "/v1/other").is_none());
}

#[test]
fn test_routing_variable_capture() {
    let mut router: Router<&str> = Router::new();
    // Pattern: /v1/messages/{id}
    // Ops: LitPush(v1), LitPush(messages), Push, Capture(id)
    let pattern = Pattern {
        ops: vec![
            Op {
                code: OpCode::LitPush,
                operand: 0,
            },
            Op {
                code: OpCode::LitPush,
                operand: 1,
            },
            Op {
                code: OpCode::Push,
                operand: 0,
            },
            Op {
                code: OpCode::Capture,
                operand: 0,
            },
        ],
        pool: vec!["v1".to_string(), "messages".to_string()],
        vars: vec!["id".to_string()],
        stack_size: 3,
        tail_len: 0,
        verb: None,
    };

    route(&mut router, Method::GET, pattern, "message_handler");

    let result = router.match_request(&Method::GET, "/v1/messages/123");
    assert!(result.is_some());
    let (_, params, _) = result.unwrap();
    assert_eq!(params.get("id").map(|s| s.as_str()), Some("123"));
}

#[test]
fn test_routing_wildcard() {
    let mut router: Router<&str> = Router::new();
    // Pattern: /v1/*/details
    // Ops: LitPush(v1), Push, LitPush(details)
    let pattern = Pattern {
        ops: vec![
            Op {
                code: OpCode::LitPush,
                operand: 0,
            },
            Op {
                code: OpCode::Push,
                operand: 0,
            },
            Op {
                code: OpCode::LitPush,
                operand: 1,
            },
        ],
        pool: vec!["v1".to_string(), "details".to_string()],
        vars: vec![],
        stack_size: 3,
        tail_len: 0,
        verb: None,
    };

    route(&mut router, Method::GET, pattern, "wildcard_handler");

    let result = router.match_request(&Method::GET, "/v1/anything/details");
    assert!(result.is_some());
}
