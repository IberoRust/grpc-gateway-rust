#[cfg(test)]
mod tests {
    use quote::quote;

    // Mock resolve_type for testing since it uses syn::parse_str which might fail without context,
    // but in generator.rs it uses simple string replacement.
    // actually generator.rs has resolve_type available in the module.

    // We need to expose resolve_relative_type and GeneratorOptions for testing
    // or copy them here. Since they are private, I will add a test module inside generator.rs instead.
}
