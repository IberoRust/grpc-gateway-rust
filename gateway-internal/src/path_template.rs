/// Represents a parsed HTTP path template.
#[derive(Debug, Clone, PartialEq)]
pub struct PathTemplate {
    pub segments: Vec<Segment>,
    pub verb: Option<String>,
    pub template: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Literal(String),
    Wildcard,
    DeepWildcard,
    Variable(Variable),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub field_path: String,
    pub segments: Vec<Segment>,
}

/// Operation codes for the compiled pattern match machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Nop = 0,
    Push = 1,
    LitPush = 2,
    PushM = 3,
    ConcatN = 4,
    Capture = 5,
}

/// A compiled pattern used for matching paths.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub ops: Vec<Op>,
    pub pool: Vec<String>,
    pub vars: Vec<String>,
    pub stack_size: usize,
    pub tail_len: usize,
    pub verb: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Op {
    pub code: OpCode,
    pub operand: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_template_creation() {
        let tmpl = PathTemplate {
            segments: vec![
                Segment::Literal("v1".to_string()),
                Segment::Variable(Variable {
                    field_path: "name".to_string(),
                    segments: vec![Segment::Wildcard],
                }),
            ],
            verb: None,
            template: "/v1/{name=*}".to_string(),
        };

        assert_eq!(tmpl.segments.len(), 2);
        match &tmpl.segments[0] {
            Segment::Literal(l) => assert_eq!(l, "v1"),
            _ => panic!("Expected Literal"),
        }
    }
}
