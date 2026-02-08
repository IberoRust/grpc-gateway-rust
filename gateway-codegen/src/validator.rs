use gateway_annotations::google::protobuf_custom::FileDescriptorProto;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProtoCategory {
    ServiceDefining,
    MessageEnumOnly,
    AnnotationOption,
    ImportOnly,
}

impl ProtoCategory {
    pub fn description(&self) -> &'static str {
        match self {
            ProtoCategory::ServiceDefining => "Service-Defining Proto",
            ProtoCategory::MessageEnumOnly => "Message / Enum-Only Proto",
            ProtoCategory::AnnotationOption => "Annotation / Option Proto",
            ProtoCategory::ImportOnly => "Import-Only Proto",
        }
    }
}

pub struct ValidationResult {
    pub category: ProtoCategory,
    pub expected_output: bool,
    pub justification: String,
}

pub fn classify(file: &FileDescriptorProto) -> ValidationResult {
    if !file.service.is_empty() {
        return ValidationResult {
            category: ProtoCategory::ServiceDefining,
            expected_output: true,
            justification: "Contains service definitions; expected to generate gateway artifacts."
                .to_string(),
        };
    }

    if !file.message_type.is_empty() || !file.enum_type.is_empty() {
        return ValidationResult {
            category: ProtoCategory::MessageEnumOnly,
            expected_output: false,
            justification: "Contains only messages/enums; no gateway services to generate."
                .to_string(),
        };
    }

    if !file.extension.is_empty() {
        return ValidationResult {
            category: ProtoCategory::AnnotationOption,
            expected_output: false,
            justification: "Contains extensions only; no runtime code expected.".to_string(),
        };
    }

    // Check if it's import only (has dependency but nothing else)
    // Or just completely empty
    ValidationResult {
        category: ProtoCategory::ImportOnly,
        expected_output: false,
        justification: "Import-only or empty proto; no code generation expected.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_annotations::google::protobuf_custom::{
        DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, ServiceDescriptorProto,
    };

    #[test]
    fn test_classify_service_defining() {
        let file = FileDescriptorProto {
            service: vec![ServiceDescriptorProto::default()],
            ..Default::default()
        };
        let result = classify(&file);
        assert_eq!(result.category, ProtoCategory::ServiceDefining);
        assert!(result.expected_output);
    }

    #[test]
    fn test_classify_message_only() {
        let file = FileDescriptorProto {
            message_type: vec![DescriptorProto::default()],
            ..Default::default()
        };
        let result = classify(&file);
        assert_eq!(result.category, ProtoCategory::MessageEnumOnly);
        assert!(!result.expected_output);
    }

    #[test]
    fn test_classify_enum_only() {
        let file = FileDescriptorProto {
            enum_type: vec![EnumDescriptorProto::default()],
            ..Default::default()
        };
        let result = classify(&file);
        assert_eq!(result.category, ProtoCategory::MessageEnumOnly);
        assert!(!result.expected_output);
    }

    #[test]
    fn test_classify_annotation_option() {
        let file = FileDescriptorProto {
            extension: vec![FieldDescriptorProto::default()],
            ..Default::default()
        };
        let result = classify(&file);
        assert_eq!(result.category, ProtoCategory::AnnotationOption);
        assert!(!result.expected_output);
    }

    #[test]
    fn test_classify_import_only() {
        let file = FileDescriptorProto {
            dependency: vec!["other.proto".to_string()],
            ..Default::default()
        };
        let result = classify(&file);
        assert_eq!(result.category, ProtoCategory::ImportOnly);
        assert!(!result.expected_output);
    }

    #[test]
    fn test_classify_empty() {
        let file = FileDescriptorProto::default();
        let result = classify(&file);
        assert_eq!(result.category, ProtoCategory::ImportOnly);
        assert!(!result.expected_output);
    }
}
