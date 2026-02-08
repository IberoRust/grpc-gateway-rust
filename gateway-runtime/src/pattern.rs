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
