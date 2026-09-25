pub mod api;
pub mod client;
pub mod error;
pub mod models;
pub mod resources;

pub use client::Honcho;
pub use client::HonchoBuilder;
pub use error::Error;
pub use resources::peer::Peer;
pub use resources::session::Session;
pub use resources::session::SessionContextRequest;

pub use models::chat::{
    ChatResponse, DialecticOptions, Evidence, EvidenceConclusion, EvidenceMessageRef,
    EvidenceToolCall, ReasoningLevel, StreamChunk,
};
pub use models::context::{
    AnthropicMessage, OpenAIMessage, PeerContext, SessionContext, Summary,
};
pub use models::message::{Message, MessageCreate};
pub use models::page::{ListOptions, Page};
pub use models::peer::PeerCreate;
pub use models::session::{SessionCreate, SessionPeerConfig};

// Include the resource accessor methods on Honcho
#[allow(unused)]
use resources::client_accessors;
