use crate::body_mapping::BodyMapping;
use crate::path_template::PathTemplate;
use crate::query_parameter::QueryParamFilter;

/// Represents a binding of an HTTP method to a gRPC method.
/// Corresponds to `Binding` in `grpc-gateway-golang`.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodBinding {
    /// The HTTP method (e.g., "GET", "POST").
    pub http_method: String,

    /// The compiled path template for this binding.
    pub path_tmpl: PathTemplate,

    /// The index of this binding in the `HttpRule.additional_bindings` list (or 0 if primary).
    pub index: usize,

    /// Path parameters extracted from the path template.
    pub path_params: Vec<Parameter>,

    /// Mapping for the request body.
    pub body: Option<BodyMapping>,

    /// Mapping for the response body.
    pub response_body: Option<BodyMapping>,

    /// Filter for query parameters.
    pub query_param_filter: Option<QueryParamFilter>,
}

/// A path parameter extracted from the path template.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    /// The field path in the protobuf message.
    pub field_path: String,

    /// The type of the target field.
    pub target_type: String, // e.g. "TYPE_STRING", "TYPE_INT32"

    /// Whether the target field is repeated.
    pub is_repeated: bool,

    /// The protobuf field type (e.g. 12 for TYPE_BYTES).
    pub field_type: Option<i32>,
}
