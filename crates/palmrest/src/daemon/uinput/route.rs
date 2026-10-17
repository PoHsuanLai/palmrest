//! Where the virtual devices come from: the input service, or palmrest's own uinput node.
//!
//! `Route` is itself a `DeviceSource`, so the emitter above it never knows which one it got.
//!
//! There is no fallback from the one to the other: a machine whose input service is not running
//! has no pad, and says so (INPUT-02). A fallback to the node would work exactly where the node
//! is open to the logged-in user, which is what the input service exists to end.

use super::direct::DirectSource;
use turnstile_client::{
    Broker, ClientError, Create, DeviceId, DeviceSource, Event, Made, Reason, Sent, Session,
};

/// What the command line chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Origin {
    /// The input service alone.
    #[default]
    Service,
    /// The node, whatever the service says (`--direct-uinput`, for development).
    Direct,
}

/// A session from one of two sources.
#[derive(Debug)]
pub enum Either<A, B> {
    Service(A),
    Node(B),
}

impl<A: Session, B: Session> Session for Either<A, B> {
    fn create(&mut self, request: &Create) -> Result<Made, ClientError> {
        match self {
            Self::Service(s) => s.create(request),
            Self::Node(n) => n.create(request),
        }
    }

    fn send(&mut self, device: DeviceId, events: &[Event]) -> Result<Sent, ClientError> {
        match self {
            Self::Service(s) => s.send(device, events),
            Self::Node(n) => n.send(device, events),
        }
    }

    fn destroy(&mut self, device: DeviceId) -> Result<(), ClientError> {
        match self {
            Self::Service(s) => s.destroy(device),
            Self::Node(n) => n.destroy(device),
        }
    }

    fn refusal(&mut self) -> Option<Reason> {
        match self {
            Self::Service(s) => s.refusal(),
            Self::Node(n) => n.refusal(),
        }
    }
}

/// The ways to get a session.
#[derive(Debug, Clone)]
pub enum Route<S, N> {
    /// The service alone.
    Service(S),
    /// The node alone.
    Node(N),
}

/// The route of a real run.
pub type LiveRoute = Route<Broker, DirectSource>;

impl LiveRoute {
    /// The route the command line chose.
    pub fn live(origin: Origin) -> Self {
        match origin {
            Origin::Service => Route::Service(Broker::default()),
            Origin::Direct => Route::Node(DirectSource),
        }
    }
}

impl<S: DeviceSource, N: DeviceSource> DeviceSource for Route<S, N> {
    type Session = Either<S::Session, N::Session>;

    fn open(&self) -> Result<Self::Session, ClientError> {
        match self {
            Route::Node(node) => node.open().map(Either::Node),
            Route::Service(service) => service.open().map(Either::Service),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use turnstile_client::fake::{FakeSource, OpenMode};

    fn absent() -> FakeSource {
        let source = FakeSource::new();
        source.state().open_mode = OpenMode::Absent;
        source
    }

    #[test]
    fn a_missing_service_is_not_made_up_for_by_the_node() {
        let route: Route<FakeSource, FakeSource> = Route::Service(absent());
        assert!(matches!(route.open(), Err(ClientError::Absent)));
    }

    #[test]
    fn a_working_service_makes_service_sessions_only() {
        let route: Route<FakeSource, FakeSource> = Route::Service(FakeSource::new());
        assert!(matches!(route.open(), Ok(Either::Service(_))));
    }

    #[test]
    fn the_direct_choice_makes_node_sessions_only() {
        let node = FakeSource::new();
        let route: Route<FakeSource, FakeSource> = Route::Node(node.clone());
        assert!(matches!(route.open(), Ok(Either::Node(_))));
        assert_eq!(node.state().opened, 1);
    }
}
