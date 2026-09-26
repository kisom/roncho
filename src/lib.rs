#[cfg(feature = "async")]
pub mod api;
#[cfg(feature = "blocking")]
pub mod blocking;
#[cfg(feature = "async")]
pub mod client;
pub mod error;
pub mod models;
#[cfg(feature = "async")]
pub mod resources;
pub(crate) mod sse;

#[cfg(feature = "async")]
pub use client::Honcho;
#[cfg(feature = "async")]
pub use client::HonchoBuilder;
pub use error::Error;
#[cfg(feature = "async")]
pub use resources::conclusions::Conclusions;
#[cfg(feature = "async")]
pub use resources::peer::Peer;
#[cfg(feature = "async")]
pub use resources::session::Session;
#[cfg(feature = "async")]
pub use resources::session::SessionContextRequest;
#[cfg(feature = "async")]
pub use resources::workspace::Workspaces;

pub use models::chat::{
    ChatResponse, DialecticOptions, Evidence, EvidenceMessageRef, EvidenceObservation,
    EvidenceToolCall, ReasoningLevel, ScopeNames, StreamChunk,
};
pub use models::conclusions::{
    Conclusion, ConclusionBatchCreate, ConclusionCreate, ConclusionListOptions, ConclusionQuery,
    Level,
};
pub use models::context::{AnthropicMessage, OpenAIMessage, PeerContext, SessionContext, Summary};
pub use models::message::{Message, MessageCreate, MessageSearch};
pub use models::page::{ListOptions, Page};
pub use models::peer::PeerCreate;
pub use models::session::{SessionCreate, SessionPeerConfig};
pub use models::workspace::{Workspace, WorkspaceCreate, WorkspaceListOptions, WorkspaceUpdate};

// Include the resource accessor methods on Honcho
#[cfg(feature = "async")]
#[allow(unused)]
use resources::client_accessors;
