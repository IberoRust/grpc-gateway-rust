// Derived from grpc-gateway-golang/runtime/pattern_test.go
// Section 5.3 Protobuf Analysis (inferred usage of runtime pattern)

use gateway_internal::path_template::{Op, OpCode, Pattern};
use gateway_runtime::pattern::route_matcher;

const ANYTHING: i32 = 0;

fn make_ops(ops: &[(OpCode, i32)]) -> Vec<Op> {
    ops.iter()
        .map(|(code, operand)| Op {
            code: *code,
            operand: *operand,
        })
        .collect()
}

#[test]
fn test_match_literal() {
    let pattern = Pattern {
        ops: make_ops(&[(OpCode::LitPush, 0)]),
        pool: vec!["v1".to_string()],
        vars: vec![],
        stack_size: 1,
        tail_len: 0,
        verb: None,
    };

    assert!(route_matcher(&pattern, "/v1").is_some());
    assert!(route_matcher(&pattern, "/v2").is_none());
}

#[test]
fn test_match_wildcard() {
    let pattern = Pattern {
        ops: make_ops(&[(OpCode::Push, ANYTHING)]),
        pool: vec![],
        vars: vec![],
        stack_size: 1,
        tail_len: 0,
        verb: None,
    };

    assert!(route_matcher(&pattern, "/abc").is_some());
    assert!(route_matcher(&pattern, "/def").is_some());
    assert!(route_matcher(&pattern, "/abc/def").is_none());
}

#[test]
fn test_match_deep_wildcard() {
    let pattern = Pattern {
        ops: make_ops(&[(OpCode::PushM, ANYTHING)]),
        pool: vec![],
        vars: vec![],
        stack_size: 1,
        tail_len: 0,
        verb: None,
    };

    assert!(route_matcher(&pattern, "/abc").is_some());
    assert!(route_matcher(&pattern, "/abc/def").is_some());
}

#[test]
fn test_match_capture_complex() {
    let pattern = Pattern {
        ops: make_ops(&[
            (OpCode::LitPush, 0),      // v1
            (OpCode::LitPush, 1),      // o
            (OpCode::PushM, ANYTHING), // matches "my-bucket/dir/dir2/obj"
            (OpCode::ConcatN, 2),      // joins "o" and "..." -> "o/..."
            (OpCode::Capture, 0),      // capture into vars[0] ("name")
        ]),
        pool: vec!["v1".to_string(), "o".to_string()],
        vars: vec!["name".to_string()],
        stack_size: 2,
        tail_len: 0,
        verb: None,
    };

    let res = route_matcher(&pattern, "/v1/o/my-bucket/dir/dir2/obj");
    assert!(res.is_some());
    let map = res.unwrap();
    assert_eq!(
        map.get("name").map(|s| s.as_str()),
        Some("o/my-bucket/dir/dir2/obj")
    );
}
