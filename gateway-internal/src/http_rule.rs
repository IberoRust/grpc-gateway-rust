/// Represents a rule that maps an RPC method to one or more HTTP REST API methods.
/// This corresponds to `google.api.HttpRule`.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRule {
    /// Selects a method to which this rule applies.
    pub selector: String,

    /// The pattern for the rule.
    pub pattern: Pattern,

    /// The name of the request field whose value is mapped to the HTTP request body.
    pub body: String,

    /// The name of the response field whose value is mapped to the HTTP response body.
    pub response_body: String,

    /// Additional HTTP bindings for the selector.
    pub additional_bindings: Vec<HttpRule>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_rule_creation() {
        let rule = HttpRule {
            selector: "example.Service.Method".to_string(),
            pattern: Pattern::Get("/v1/example".to_string()),
            body: "*".to_string(),
            response_body: "".to_string(),
            additional_bindings: vec![],
        };

        assert_eq!(rule.selector, "example.Service.Method");
        match rule.pattern {
            Pattern::Get(path) => assert_eq!(path, "/v1/example"),
            _ => panic!("Expected GET pattern"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Get(String),
    Put(String),
    Post(String),
    Delete(String),
    Patch(String),
    Custom { kind: String, path: String },
}
