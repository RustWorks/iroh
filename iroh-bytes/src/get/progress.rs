//! Types for get progress state management.

use std::{collections::HashMap, num::NonZeroU64};

use serde::{Deserialize, Serialize};

use crate::{protocol::RangeSpec, store::BaoBlobSize, Hash};

use super::db::DownloadProgress;

/// The progress identifier for individual blobs.
pub type ProgressId = u64;

/// Progress state of a transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferState {
    /// The root blob of this transfer (may be a hash seq),
    pub root: BlobState,
    /// Whether we are connected to a node
    pub connected: bool,
    /// Children if the root blob is a hash seq, empty for raw blobs
    pub children: HashMap<NonZeroU64, BlobState>,
    /// Child being transferred at the moment.
    pub current: Option<BlobId>,
    /// Progress ids for individual blobs.
    pub progress_ids: HashMap<ProgressId, BlobId>,
}

impl TransferState {
    /// Create a new, empty transfer state.
    pub fn new(root_hash: Hash) -> Self {
        Self {
            root: BlobState::new(root_hash),
            connected: false,
            children: Default::default(),
            current: None,
            progress_ids: Default::default(),
        }
    }
}

/// State of a single blob in transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobState {
    /// The hash of this blob.
    pub hash: Hash,
    /// The size of this blob. Only known if the blob is partially present locally, or after having
    /// received the size from the remote.
    pub size: Option<BaoBlobSize>,
    /// The current state of the blob transfer.
    pub progress: ProgressState,
    /// Ranges already available locally at the time of starting the transfer.
    pub local_ranges: Option<RangeSpec>,
    /// Number of children (only applies to hashseqs, None for raw blobs).
    pub child_count: Option<u64>,
}

/// Progress state for a single blob
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum ProgressState {
    /// Download is pending
    #[default]
    Pending,
    /// Download is in progress
    Progressing(u64),
    /// Download has finished
    Done,
}

impl BlobState {
    /// Create a new [`BlobState`].
    pub fn new(hash: Hash) -> Self {
        Self {
            hash,
            size: None,
            local_ranges: None,
            child_count: None,
            progress: ProgressState::default(),
        }
    }
}

impl TransferState {
    /// Get state of the root blob of this transfer.
    pub fn root(&self) -> &BlobState {
        &self.root
    }

    /// Get a blob state by its [`BlobId`] in this transfer.
    pub fn get_blob(&self, blob_id: &BlobId) -> Option<&BlobState> {
        match blob_id {
            BlobId::Root => Some(&self.root),
            BlobId::Child(id) => self.children.get(id),
        }
    }

    /// Get the blob state currently being transferred.
    pub fn get_current(&self) -> Option<&BlobState> {
        self.current.as_ref().and_then(|id| self.get_blob(id))
    }

    fn get_or_insert_blob(&mut self, blob_id: BlobId, hash: Hash) -> &mut BlobState {
        match blob_id {
            BlobId::Root => &mut self.root,
            BlobId::Child(id) => self
                .children
                .entry(id)
                .or_insert_with(|| BlobState::new(hash)),
        }
    }
    fn get_blob_mut(&mut self, blob_id: &BlobId) -> Option<&mut BlobState> {
        match blob_id {
            BlobId::Root => Some(&mut self.root),
            BlobId::Child(id) => self.children.get_mut(id),
        }
    }

    fn get_by_progress_id(&mut self, progress_id: ProgressId) -> Option<&mut BlobState> {
        let blob_id = *self.progress_ids.get(&progress_id)?;
        self.get_blob_mut(&blob_id)
    }

    /// Update the state with a new [`DownloadProgress`] event for this transfer.
    pub fn on_progress(&mut self, event: DownloadProgress) {
        match event {
            DownloadProgress::FoundLocal {
                child,
                hash,
                size,
                valid_ranges,
            } => {
                let blob = self.get_or_insert_blob(BlobId::from_child_id(child), hash);
                blob.size = Some(size);
                blob.local_ranges = Some(valid_ranges);
            }
            DownloadProgress::Connected => self.connected = true,
            DownloadProgress::Found {
                id: progress_id,
                child,
                hash,
                size,
            } => {
                let blob_id = BlobId::from_child_id(child);
                let blob = self.get_or_insert_blob(blob_id, hash);
                if blob.size.is_none() {
                    blob.size = Some(BaoBlobSize::Verified(size));
                }
                blob.progress = ProgressState::Progressing(0);
                self.progress_ids.insert(progress_id, blob_id);
                self.current = Some(blob_id);
            }
            DownloadProgress::FoundHashSeq { hash, children } => {
                if hash == self.root.hash {
                    self.root.child_count = Some(children);
                } else {
                    // I think it is an invariant of the protocol that `FoundHashSeq` is only
                    // triggered for the root hash.
                }
            }
            DownloadProgress::Progress { id, offset } => {
                if let Some(blob) = self.get_by_progress_id(id) {
                    blob.progress = ProgressState::Progressing(offset);
                }
            }
            DownloadProgress::Done { id } => {
                if let Some(blob) = self.get_by_progress_id(id) {
                    blob.progress = ProgressState::Done;
                    self.progress_ids.remove(&id);
                }
            }
            _ => {}
        }
    }
}

/// The id of a blob in a transfer
#[derive(
    Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, std::hash::Hash, Serialize, Deserialize,
)]
pub enum BlobId {
    /// The root blob (child id 0)
    Root,
    /// A child blob (child id > 0)
    Child(NonZeroU64),
}

impl BlobId {
    fn from_child_id(id: u64) -> Self {
        match id {
            0 => BlobId::Root,
            _ => BlobId::Child(NonZeroU64::new(id).expect("just checked")),
        }
    }
}

impl From<BlobId> for u64 {
    fn from(value: BlobId) -> Self {
        match value {
            BlobId::Root => 0,
            BlobId::Child(id) => id.into(),
        }
    }
}
