//! Runtime library for the Lucy documentation framework.
//!
//! Provides the global endpoint registry, JSON spec generation,
//! the Axum router serving `/docs`, and static UI asset embedding.
//!
//! # Quick start
//! ```rust,ignore
//! use lucy_core::{EndpointRegistry, docs_router};
//!
//! let app = axum::Router::new().merge(docs_router());
//! ```

pub mod assets;
pub mod registry;
pub mod router;
pub mod spec;

pub use registry::EndpointRegistry;
pub use router::docs_router;
