use mockito::{Server, ServerGuard};
use std::panic;

pub use mockito::Matcher;

/// Helper to start a mock server safely in sandboxed environments
pub fn start_mock_server(test_name: &str) -> Option<ServerGuard> {
    match panic::catch_unwind(Server::new) {
        Ok(server) => Some(server),
        Err(_) => {
            eprintln!(
                "skipping {test_name} - unable to start mock server (sandbox may restrict networking)"
            );
            None
        }
    }
}
