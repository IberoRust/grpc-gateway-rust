//! # Pattern Matching
//!
//! ## Purpose
//! Implements the path matching logic used by the router. It interprets the compiled
//! path patterns (from `gateway-internal`) and checks if an incoming request path
//! matches a defined route, extracting path parameters in the process.
//!
//! ## Scope
//! This module defines:
//! -   `route_matcher`: The core function that matches a path against a `Pattern`.
//!
//! ## Position in the Architecture
//! Used exclusively by `Router::match_request`.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use gateway_internal::path_template::{OpCode, Pattern};

/// Matches a request path against a compiled pattern.
///
/// # Parameters
/// *   `pattern`: The compiled path pattern.
/// *   `path`: The request path string.
///
/// # Returns
/// An `Option` containing a map of captured variables if the path matches.
/// Returns `None` if there is no match.
pub fn route_matcher(pattern: &Pattern, path: &str) -> Option<BTreeMap<String, String>> {
    let path = path.strip_prefix('/').unwrap_or(path);
    let mut components: Vec<&str> = path.split('/').collect();

    // Remove empty trailing component if path ends with /
    if let Some(last) = components.last() {
        if last.is_empty() && components.len() > 1 {
            components.pop();
        }
    }

    let verb = pattern.verb.as_deref().unwrap_or("");
    let mut current_verb = "";

    if let Some(last) = components.last() {
        if let Some(idx) = last.rfind(':') {
            let (_head, v) = last.split_at(idx);
            if let Some(stripped) = v.strip_prefix(':') {
                if stripped == verb {
                    current_verb = stripped;
                }
            }
        }
    }

    if verb != current_verb {
        return None;
    }

    if !verb.is_empty() {
        let last_idx = components.len() - 1;
        let last = components[last_idx];
        let (head, _) = last.split_at(last.len() - verb.len() - 1);
        components[last_idx] = head;
    }

    let mut stack: Vec<String> = Vec::with_capacity(pattern.stack_size as usize);
    let mut captured: BTreeMap<String, String> = BTreeMap::new();
    let mut pos = 0;

    for op in &pattern.ops {
        match op.code {
            OpCode::Nop => continue,
            OpCode::Push | OpCode::LitPush => {
                if pos >= components.len() {
                    return None;
                }
                let c = components[pos];
                if op.code == OpCode::LitPush {
                    if let Some(lit) = pattern.pool.get(op.operand as usize) {
                        if c != lit {
                            return None;
                        }
                    } else {
                        return None; // Invalid pool index
                    }
                }
                stack.push(c.to_string());
                pos += 1;
            }
            OpCode::PushM => {
                if components.len() < pos + pattern.tail_len as usize {
                    return None;
                }
                let end = components.len() - pattern.tail_len as usize;
                let c = components[pos..end].join("/");
                stack.push(c);
                pos = end;
            }
            OpCode::ConcatN => {
                let n = op.operand as usize;
                if stack.len() < n {
                    return None;
                }
                let split_idx = stack.len() - n;
                let joined = stack[split_idx..].join("/");
                stack.truncate(split_idx);
                stack.push(joined);
            }
            OpCode::Capture => {
                if let Some(val) = stack.pop() {
                    if let Some(var_name) = pattern.vars.get(op.operand as usize) {
                        captured.insert(var_name.clone(), val);
                    }
                } else {
                    return None; // Stack underflow
                }
            }
        }
    }

    if pos < components.len() {
        return None;
    }

    Some(captured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_internal::path_template::Op;

    #[test]
    fn test_match_literal() {
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        assert!(route_matcher(&pattern, "/foo").is_some());
        assert!(route_matcher(&pattern, "/bar").is_none());
    }

    #[test]
    fn test_match_capture() {
        let pattern = Pattern {
            ops: vec![
                Op {
                    code: OpCode::Push,
                    operand: 0,
                },
                Op {
                    code: OpCode::Capture,
                    operand: 0,
                },
            ],
            pool: vec![],
            vars: vec!["id".to_string()],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        let res = route_matcher(&pattern, "/123");
        assert!(res.is_some());
        assert_eq!(res.unwrap().get("id").unwrap(), "123");
    }

    #[test]
    fn test_match_wildcard() {
        let pattern = Pattern {
            ops: vec![
                Op {
                    code: OpCode::Push,
                    operand: 0,
                },
                Op {
                    code: OpCode::Capture,
                    operand: 0,
                },
            ],
            pool: vec![],
            vars: vec!["id".to_string()],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        assert!(route_matcher(&pattern, "/anything").is_some());
    }

    #[test]
    fn test_match_deep_wildcard() {
        // Pattern: /foo/** -> LitPush(0), PushM
        let pattern = Pattern {
            ops: vec![
                Op {
                    code: OpCode::LitPush,
                    operand: 0,
                },
                Op {
                    code: OpCode::PushM,
                    operand: 0,
                },
            ],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 2,
            tail_len: 0, // PushM consumes rest, tail_len is what comes AFTER PushM. 0 here.
            verb: None,
        };
        assert!(route_matcher(&pattern, "/foo/bar/baz").is_some());
        assert!(route_matcher(&pattern, "/bar/baz").is_none());
    }

    #[test]
    fn test_match_verb() {
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: Some("verb".to_string()),
        };
        assert!(route_matcher(&pattern, "/foo:verb").is_some());
        assert!(route_matcher(&pattern, "/foo").is_none());
    }

    #[test]
    fn test_match_empty() {
        // Empty pattern matches /?
        let _pattern = Pattern {
            ops: vec![],
            pool: vec![],
            vars: vec![],
            stack_size: 0,
            tail_len: 0,
            verb: None,
        };
        // Empty pattern usually matches root if ops are empty?
        // Logic: loop ops. If empty ops, loop finishes.
        // Check pos < components.len().
        // Path "/" -> components ["", ""].
        // Path "a" -> ["a"].
        // If path is empty, components is empty?
        // strip_prefix('/').
        // if path is "", it remains "". split gives [""].
        // components len 1. pos 0. pos < len? 0 < 1. Returns None.
        // So empty pattern matches nothing unless empty path provided?
        // Usually root is matches by LitPush("")? Or just empty logic?
        // We won't test empty pattern undefined behavior.

        // Test root path matching
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["".to_string()], // Matches empty component?
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        // Path "/" -> split -> [""] (after pop if last empty).
        // split("/") -> ["", ""]. last is empty. pop -> [""].
        // LitPush matches ""?
        // c = "". lit = "". Match.
        assert!(route_matcher(&pattern, "/").is_some());
    }

    #[test]
    fn test_match_trailing_slash() {
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        assert!(route_matcher(&pattern, "/foo/").is_some());
    }

    #[test]
    fn test_match_concat_n() {
        // Pattern: /foo/{a=**} -> LitPush(foo), PushM, Capture(a) ?
        // Or if we have complex variable: /v1/{name=projects/*/locations/*}
        // That involves ConcatN.
        // Let's simulate ConcatN.
        // Stack: [a, b, c]. ConcatN(3) -> "a/b/c".

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
                    code: OpCode::ConcatN,
                    operand: 2,
                },
                Op {
                    code: OpCode::Capture,
                    operand: 0,
                },
            ],
            pool: vec!["a".to_string(), "b".to_string()],
            vars: vec!["v".to_string()],
            stack_size: 2,
            tail_len: 0,
            verb: None,
        };

        // Matches /a/b.
        // 1. LitPush(a). Stack [a].
        // 2. LitPush(b). Stack [a, b].
        // 3. ConcatN(2). Stack ["a/b"].
        // 4. Capture(v). captured["v"] = "a/b".

        let res = route_matcher(&pattern, "/a/b");
        assert!(res.is_some());
        assert_eq!(res.unwrap().get("v").unwrap(), "a/b");
    }

    #[test]
    fn test_match_fail_literal() {
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        assert!(route_matcher(&pattern, "/bar").is_none());
    }

    #[test]
    fn test_match_fail_length() {
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        assert!(route_matcher(&pattern, "/foo/bar").is_none());
    }
}
