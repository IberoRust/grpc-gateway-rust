pub mod google {
    pub mod api {
        include!(concat!(env!("OUT_DIR"), "/google.api.rs"));
    }
    pub mod protobuf_custom {
        include!(concat!(env!("OUT_DIR"), "/google.protobuf_custom.rs"));
        pub mod compiler {
            include!(concat!(
            env!("OUT_DIR"),
            "/google.protobuf_custom.compiler.rs"
            ));
        }
    }
}

use google::api::HttpRule;
use google::protobuf_custom::MethodOptions;

pub fn get_http_rule(options: &MethodOptions) -> Option<HttpRule> {
    options.http.clone()
}
