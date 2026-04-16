//! Global endpoint registry.
//!
//! The registry is the in-memory source of truth for every endpoint
//! annotated with the Lucy proc-macros. It is stored behind a
//! [`OnceLock`]-initialised [`Mutex`] so that generated `#[ctor]`-style
//! registration code can safely populate it from multiple threads at
//! program start-up.

use lucy_types::endpoint::EndpointMeta;
use std::sync::{Mutex, OnceLock};

/// Global singleton registry, initialised once at startup.
///
/// Uses `OnceLock<Mutex<...>>` for safe concurrent access: the
/// [`OnceLock`] guarantees a single initialisation, and the [`Mutex`]
/// serialises mutations from registration sites.
static REGISTRY: OnceLock<Mutex<EndpointRegistry>> = OnceLock::new();

/// Returns the global [`EndpointRegistry`], initialising it on first call.
///
/// The returned reference has `'static` lifetime and is safe to share
/// across threads. Callers must acquire the inner [`Mutex`] lock before
/// reading or writing the registry.
pub fn global_registry() -> &'static Mutex<EndpointRegistry> {
    REGISTRY.get_or_init(|| Mutex::new(EndpointRegistry::new()))
}

/// Stores all annotated endpoints discovered at startup.
///
/// The registry is append-only: entries are pushed in registration order
/// and never removed, which keeps iteration deterministic and avoids any
/// need for interior indexing.
pub struct EndpointRegistry {
    endpoints: Vec<EndpointMeta>,
}

impl EndpointRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            endpoints: Vec::new(),
        }
    }

    /// Registers a new endpoint.
    ///
    /// This method is intended to be invoked by proc-macro generated
    /// code at program start-up, but it is equally safe to call by hand
    /// for tests or custom registration flows.
    pub fn register(&mut self, meta: EndpointMeta) {
        self.endpoints.push(meta);
    }

    /// Returns a slice of all registered endpoints, in registration order.
    pub fn all(&self) -> &[EndpointMeta] {
        &self.endpoints
    }
}

impl Default for EndpointRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lucy_types::endpoint::Protocol;

    // Named fixtures so the intent of each test stays visible without
    // repeating string literals everywhere.
    const FIRST_NAME: &str = "first";
    const FIRST_PATH: &str = "/first";
    const SECOND_NAME: &str = "second";
    const SECOND_PATH: &str = "/second";

    #[test]
    fn new_produces_empty_registry() {
        let registry = EndpointRegistry::new();
        assert!(
            registry.all().is_empty(),
            "a freshly-constructed registry must contain no endpoints"
        );
    }

    #[test]
    fn register_appends_entries_and_all_returns_them_in_order() {
        let mut registry = EndpointRegistry::new();

        registry.register(EndpointMeta::new(FIRST_NAME, FIRST_PATH, Protocol::Http));
        registry.register(EndpointMeta::new(
            SECOND_NAME,
            SECOND_PATH,
            Protocol::WebSocket,
        ));

        let all = registry.all();
        assert_eq!(all.len(), 2, "two registrations must yield two entries");
        assert_eq!(all[0].name, FIRST_NAME);
        assert_eq!(all[1].name, SECOND_NAME);
    }

    #[test]
    fn default_matches_new() {
        let default_registry = EndpointRegistry::default();
        assert!(
            default_registry.all().is_empty(),
            "Default impl must behave identically to EndpointRegistry::new"
        );
    }
}
