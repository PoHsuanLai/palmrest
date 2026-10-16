//! Where the virtual devices come from: the input service, or palmrest's own uinput node.
//!
//! `Route` is itself a `DeviceSource`, so the emitter above it never knows which one it got.
//!
//! TRANSITION: this release prefers the service and falls back to the node when there is no
//! service and the node can be opened, with a warning. The next release removes the fallback
//! (and the `ServiceThenNode` arm); `--direct-uinput` stays for development. FINDINGS says so.

use super::direct::DirectSource;
use turnstile_client::{
    Broker, ClientError, Create, DeviceId, DeviceSource, Event, Made, Reason, Sent, Session,
};

/// What the command line chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Origin {
    /// The input service; this release still falls back to the node when there is no service.
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
    /// The service first; the node only when there is no service and the node opens.
    ServiceThenNode { service: S, node: N },
    /// The node alone.
    Node(N),
}

/// The route of a real run.
pub type LiveRoute = Route<Broker, DirectSource>;

impl LiveRoute {
    /// The route the command line chose.
    pub fn live(origin: Origin) -> Self {
        match origin {
            Origin::Service => Route::ServiceThenNode {
                service: Broker::default(),
                node: DirectSource,
            },
            Origin::Direct => Route::Node(DirectSource),
        }
    }
}

impl<S: DeviceSource, N: DeviceSource> DeviceSource for Route<S, N> {
    type Session = Either<S::Session, N::Session>;

    fn open(&self) -> Result<Self::Session, ClientError> {
        match self {
            Route::Node(node) => node.open().map(Either::Node),
            Route::ServiceThenNode { service, node } => match service.open() {
                Err(ClientError::Absent) => match node.open() {
                    Ok(session) => {
                        eprintln!(
                            "palmrest: warning: the input service is not running, so palmrest \
                             makes its virtual devices itself; the next release will not"
                        );
                        Ok(Either::Node(session))
                    }
                    // The node is closed to us as well: the missing service is the news.
                    Err(_) => Err(ClientError::Absent),
                },
                other => other.map(Either::Service),
            },
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
    fn a_missing_service_falls_back_only_to_a_node_that_opens() {
        let route = Route::ServiceThenNode {
            service: absent(),
            node: FakeSource::new(),
        };
        assert!(matches!(route.open(), Ok(Either::Node(_))));

        let closed = Route::ServiceThenNode {
            service: absent(),
            node: absent(),
        };
        assert!(matches!(closed.open(), Err(ClientError::Absent)));
    }

    #[test]
    fn a_working_service_is_used_and_the_node_is_never_touched() {
        let node = FakeSource::new();
        let route = Route::ServiceThenNode {
            service: FakeSource::new(),
            node: node.clone(),
        };
        assert!(matches!(route.open(), Ok(Either::Service(_))));
        assert_eq!(node.state().opened, 0);
    }

    #[test]
    fn the_direct_choice_makes_node_sessions_only() {
        let node = FakeSource::new();
        let route: Route<FakeSource, FakeSource> = Route::Node(node.clone());
        assert!(matches!(route.open(), Ok(Either::Node(_))));
        assert_eq!(node.state().opened, 1);
    }
}
