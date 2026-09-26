pub mod api;
pub mod client;
pub mod error;
pub mod models;
pub mod resources;

pub use client::Honcho;
pub use client::HonchoBuilder;
pub use error::Error;
pub use resources::conclusions::Conclusions;
pub use resources::peer::Peer;
pub use resources::session::Session;
pub use resources::session::SessionContextRequest;
pub use resources::workspace::Workspaces;

pub use models::chat::{
    ChatResponse, DialecticOptions, Evidence, EvidenceMessageRef, EvidenceToolCall, ReasoningLevel,
    StreamChunk,
};
pub use models::conclusions::{
    Conclusion, ConclusionBatchCreate, ConclusionCreate, ConclusionListOptions, ConclusionQuery,
    Level,
};
pub use models::context::{AnthropicMessage, OpenAIMessage, PeerContext, SessionContext, Summary};
pub use models::message::{Message, MessageCreate};
pub use models::page::{ListOptions, Page};
pub use models::peer::PeerCreate;
pub use models::session::{SessionCreate, SessionPeerConfig};
pub use models::workspace::{Workspace, WorkspaceCreate, WorkspaceListOptions, WorkspaceUpdate};

// Include the resource accessor methods on Honcho
#[allow(unused)]
use resources::client_accessors;
