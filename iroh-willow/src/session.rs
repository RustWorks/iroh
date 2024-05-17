use std::collections::{HashMap, HashSet};

use crate::proto::grouping::Area;
use crate::proto::keys::NamespaceId;
use crate::proto::wgps::{AccessChallenge, AreaOfInterestHandle, ChallengeHash};
use crate::proto::{grouping::AreaOfInterest, wgps::ReadCapability};

pub mod channels;
mod error;
mod reconciler;
mod resource;
mod run;
mod state;
mod util;

pub use self::channels::Channels;
pub use self::error::Error;
pub use self::state::Session;

/// Data from the initial transmission
///
/// This happens before the session is initialized.
#[derive(Debug)]
pub struct InitialTransmission {
    /// The [`AccessChallenge`] nonce, whose hash we sent to the remote.
    pub our_nonce: AccessChallenge,
    /// The [`ChallengeHash`] we received from the remote.
    pub received_commitment: ChallengeHash,
    /// The maximum payload size we received from the remote.
    pub their_max_payload_size: u64,
}

/// To break symmetry, we refer to the peer that initiated the synchronisation session as Alfie,
/// and the other peer as Betty.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Role {
    /// The peer that initiated the synchronisation session.
    Alfie,
    /// The peer that accepted the synchronisation session.
    Betty,
}

impl Role {
    /// Returns `true` if we initiated the session.
    pub fn is_alfie(&self) -> bool {
        matches!(self, Role::Alfie)
    }
    /// Returns `true` if we accepted the session.
    pub fn is_betty(&self) -> bool {
        matches!(self, Role::Betty)
    }
}

/// Options to initialize a session with.
#[derive(Debug)]
pub struct SessionInit {
    /// List of interests we wish to synchronize, together with our capabilities to read them.
    pub interests: HashMap<ReadCapability, HashSet<AreaOfInterest>>,
}

impl SessionInit {
    /// Returns a [`SessionInit`] with a single interest.
    pub fn with_interest(capability: ReadCapability, area_of_interest: AreaOfInterest) -> Self {
        Self {
            interests: HashMap::from_iter([(capability, HashSet::from_iter([area_of_interest]))]),
        }
    }
}

/// The bind scope for resources.
///
/// Resources are bound by either peer
#[derive(Copy, Clone, Debug)]
pub enum Scope {
    /// Resources bound by ourselves.
    Ours,
    /// Resources bound by the other peer.
    Theirs,
}

/// Intersection between two areas of interest.
#[derive(Debug, Clone)]
pub struct AreaOfInterestIntersection {
    pub our_handle: AreaOfInterestHandle,
    pub their_handle: AreaOfInterestHandle,
    pub intersection: Area,
    pub namespace: NamespaceId,
}

