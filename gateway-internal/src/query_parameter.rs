/// Represents a filter for query parameters, preventing them from being mapped if they are
/// already handled by path parameters or the body.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryParamFilter {
    /// Encoding of the field paths.
    /// In Go implementation, this uses a DoubleArray trie structure for efficiency.
    /// Here we represent it as a set of field paths for simplicity in the domain model phase.
    pub field_paths: Vec<String>,
}

/// A mapping from a query parameter key to a protobuf field path.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryParameter {
    /// The key in the query string (e.g., "page_size").
    pub key: String,

    /// The target field path in the protobuf message (e.g., "page_size").
    pub field_path: Vec<String>,
}
