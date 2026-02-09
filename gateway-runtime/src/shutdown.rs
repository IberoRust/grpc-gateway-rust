//! # Shutdown
//!
//! ## Purpose
//! Provides utilities for graceful shutdown of the gateway server.
//!
//! ## Scope
//! This module defines:
//! -   `GracefulShutdown`: A struct to manage shutdown signals and coordination.
//! -   `wait_for_signal`: A helper to wait for SIGINT/SIGTERM.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

#[cfg(feature = "std")]
use tokio::signal;

/// A future that resolves when a shutdown signal is received.
pub struct ShutdownSignal {
    #[cfg(feature = "std")]
    inner: Pin<Box<dyn Future<Output = ()> + Send + Sync>>,
}

impl ShutdownSignal {
    /// Creates a new shutdown signal listener.
    pub fn new() -> Self {
        #[cfg(feature = "std")]
        {
            let fut = async {
                let ctrl_c = async {
                    signal::ctrl_c()
                        .await
                        .expect("failed to install Ctrl+C handler");
                };

                #[cfg(unix)]
                let terminate = async {
                    signal::unix::signal(signal::unix::SignalKind::terminate())
                        .expect("failed to install signal handler")
                        .recv()
                        .await;
                };

                #[cfg(not(unix))]
                let terminate = std::future::pending::<()>();

                tokio::select! {
                    _ = ctrl_c => {},
                    _ = terminate => {},
                }
            };
            Self {
                inner: Box::pin(fut),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            Self {}
        }
    }
}

impl Future for ShutdownSignal {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[cfg(feature = "std")]
        return self.inner.as_mut().poll(cx);
        #[cfg(not(feature = "std"))]
        Poll::Pending
    }
}

/// Helper function to wait for a shutdown signal.
pub async fn wait_for_signal() {
    ShutdownSignal::new().await
}
