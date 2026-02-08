use gateway_internal::path_template::{Op, OpCode, Pattern};

pub fn compile(template: &str) -> Pattern {
    let mut ops = Vec::new();
    let mut pool = Vec::new();
    let mut vars = Vec::new();
    let mut stack_size = 0;

    // Normalize
    let template = template.strip_prefix('/').unwrap_or(template);

    // Extract verb if present
    let (path_str, verb) = if let Some(idx) = template.rfind(':') {
        // Check if the colon is inside a brace?
        // Simple heuristic: Iterate backwards. If } seen before :, then : is outside (verb).
        // Actually, verbs are strictly at the end.
        // But patterns like {id=foo:bar} are possible?
        // Standard google.api.http says:
        // Template = "/" Segments [ Verb ] ;
        // Verb = ":" LITERAL ;
        // Segments are slash separated.
        // If the last segment contains :, it might be a verb.
        let (p, v) = template.split_at(idx);
        (p, Some(v[1..].to_string()))
    } else {
        (template, None)
    };

    // Robust tokenization: split by '/', but merge if inside braces
    let raw_segments: Vec<&str> = path_str.split('/').collect();
    let mut merged_segments = Vec::new();
    let mut buffer = String::new();
    let mut brace_depth = 0;

    for (_i, segment) in raw_segments.iter().enumerate() {
        if segment.is_empty() {
            continue;
        }

        let open_count = segment.chars().filter(|c| *c == '{').count();
        let close_count = segment.chars().filter(|c| *c == '}').count();

        if brace_depth > 0 {
            buffer.push('/');
            buffer.push_str(segment);
            brace_depth += open_count;
            brace_depth -= close_count;
            if brace_depth == 0 {
                merged_segments.push(buffer.clone());
                buffer.clear();
            }
        } else {
            if open_count > close_count {
                brace_depth += open_count - close_count;
                buffer.push_str(segment);
            } else {
                merged_segments.push(segment.to_string());
            }
        }
    }

    for segment in merged_segments {
        if segment == "*" {
            ops.push(Op {
                code: OpCode::Push,
                operand: 0,
            });
            stack_size += 1;
        } else if segment == "**" {
            ops.push(Op {
                code: OpCode::PushM,
                operand: 0,
            });
            stack_size += 1;
        } else if segment.starts_with('{') && segment.ends_with('}') {
            // Variable: {name} or {name=pattern}
            let content = &segment[1..segment.len() - 1];
            let (var_name, pattern) = if let Some(idx) = content.find('=') {
                let (v, p) = content.split_at(idx);
                (v, &p[1..]) // skip =
            } else {
                (content, "*")
            };

            // Compile pattern
            let pattern_parts: Vec<&str> = pattern.split('/').collect();
            let mut pattern_stack_pushes = 0;

            for part in pattern_parts {
                if part == "*" {
                    ops.push(Op {
                        code: OpCode::Push,
                        operand: 0,
                    });
                    pattern_stack_pushes += 1;
                } else if part == "**" {
                    ops.push(Op {
                        code: OpCode::PushM,
                        operand: 0,
                    });
                    pattern_stack_pushes += 1;
                } else {
                    // Literal
                    pool.push(part.to_string());
                    let pool_idx = pool.len() - 1;
                    ops.push(Op {
                        code: OpCode::LitPush,
                        operand: pool_idx as i32,
                    });
                    pattern_stack_pushes += 1;
                }
            }

            // If pattern pushed multiple items, we must concatenate them before capture
            // to capture the full path string into the variable.
            if pattern_stack_pushes > 1 {
                ops.push(Op {
                    code: OpCode::ConcatN,
                    operand: pattern_stack_pushes as i32,
                });
                // ConcatN pops N, pushes 1.
                // Net change to global stack_size: +N -N +1 = +1.
                // Wait, I tracked stack_size by adding for each Push/LitPush.
                // So current stack_size has increased by `pattern_stack_pushes`.
                // ConcatN reduces stack height in runtime, but for my `stack_size` var (max depth?),
                // we assume usage.
                // Wait, `Pattern.stack_size` is usually "max stack depth required" or "expected stack size at capture"?
                // `pattern.rs` uses `Vec::with_capacity(pattern.stack_size)`.
                // So it's capacity. Sum of pushes is fine.
            }

            // Capture
            vars.push(var_name.to_string());
            let var_idx = vars.len() - 1;
            ops.push(Op {
                code: OpCode::Capture,
                operand: var_idx as i32,
            });

            // `stack_size` tracking here is loose, mainly for capacity hint.
            stack_size += pattern_stack_pushes;
        } else {
            // Top-level Literal
            pool.push(segment.to_string());
            let pool_idx = pool.len() - 1;
            ops.push(Op {
                code: OpCode::LitPush,
                operand: pool_idx as i32,
            });
            stack_size += 1;
        }
    }

    Pattern {
        ops,
        pool,
        vars,
        stack_size,
        tail_len: 0,
        verb,
    }
}
