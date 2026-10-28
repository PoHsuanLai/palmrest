//! Where the virtual devices come from: the input service, or palmrest's own uinput node.
//!
//! `LiveSource` is itself a `DeviceSource`, so the emitter above it never knows which one it got.
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

/// A session from whichever source the command line chose.
pub struct Live(Box<dyn Session>);

impl Session for Live {
    fn create(&mut self, request: &Create) -> Result<Made, ClientError> {
        self.0.create(request)
    }

    fn send(&mut self, device: DeviceId, events: &[Event]) -> Result<Sent, ClientError> {
        self.0.send(device, events)
    }

    fn destroy(&mut self, device: DeviceId) -> Result<(), ClientError> {
        self.0.destroy(device)
    }

    fn refusal(&mut self) -> Option<Reason> {
        self.0.refusal()
    }
}

/// The source of a real run: the service or the node, as `origin` says.
#[derive(Debug, Clone, Copy)]
pub struct LiveSource(pub Origin);

impl DeviceSource for LiveSource {
    type Session = Live;

    fn open(&self) -> Result<Live, ClientError> {
        open_from(self.0, &Broker::default(), &DirectSource)
    }
}

/// A session from `service` or from `node`, never from the other when the first is missing.
fn open_from<S: DeviceSource, N: DeviceSource>(
    origin: Origin,
    service: &S,
    node: &N,
) -> Result<Live, ClientError>
where
    S::Session: 'static,
    N::Session: 'static,
{
    match origin {
        Origin::Service => service.open().map(|s| Live(Box::new(s))),
        Origin::Direct => node.open().map(|s| Live(Box::new(s))),
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
        let node = FakeSource::new();
        assert!(matches!(
            open_from(Origin::Service, &absent(), &node),
            Err(ClientError::Absent)
        ));
        assert_eq!(node.state().opened, 0);
    }

    #[test]
    fn a_working_service_makes_service_sessions_only() {
        let (service, node) = (FakeSource::new(), FakeSource::new());
        assert!(open_from(Origin::Service, &service, &node).is_ok());
        assert_eq!((service.state().opened, node.state().opened), (1, 0));
    }

    #[test]
    fn the_direct_choice_makes_node_sessions_only() {
        let (service, node) = (FakeSource::new(), FakeSource::new());
        assert!(open_from(Origin::Direct, &service, &node).is_ok());
        assert_eq!((service.state().opened, node.state().opened), (0, 1));
    }
}
