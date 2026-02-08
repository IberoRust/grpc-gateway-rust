/// Represents how an HTTP request/response body maps to a protobuf message field.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyMapping {
    /// The field path within the protobuf message that maps to the body.
    /// If empty or "*", it maps to the entire message.
    pub field_path: Option<String>,
}

impl BodyMapping {
    pub fn is_whole_message(&self) -> bool {
        match &self.field_path {
            Some(path) => path == "*" || path.is_empty(),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_whole_message() {
        assert!(BodyMapping { field_path: None }.is_whole_message());
        assert!(BodyMapping {
            field_path: Some("*".to_string())
        }
            .is_whole_message());
        assert!(BodyMapping {
            field_path: Some("".to_string())
        }
            .is_whole_message());
        assert!(!BodyMapping {
            field_path: Some("data".to_string())
        }
            .is_whole_message());
    }
}
