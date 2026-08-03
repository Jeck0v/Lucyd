//! Runtime library for the Lucyd documentation framework.
//!
//! Provides the global endpoint registry, JSON spec generation,
//! the Axum router serving `/docs`, and static UI asset embedding.
//!
//! # Quick start
//! ```rust,ignore
//! use lucyd_core::{EndpointRegistry, docs_router};
//!
//! let app = axum::Router::new().merge(docs_router());
//! ```

pub mod assets;
pub mod openapi;
pub mod registry;
pub mod router;
pub mod spec;

pub use registry::EndpointRegistry;
pub use router::docs_router;
