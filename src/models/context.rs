use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub id: String,
    pub messages: Vec<super::message::Message>,
    #[serde(default)]
    pub summary: Option<Summary>,
    #[serde(default)]
    pub peer_representation: Option<String>,
    #[serde(default)]
    pub peer_card: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerContext {
    pub peer_id: String,
    pub target_id: String,
    #[serde(default)]
    pub representation: Option<String>,
    #[serde(default)]
    pub peer_card: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub content: String,
    #[serde(rename = "message_id")]
    pub message_id: String,
    #[serde(rename = "summary_type")]
    pub summary_type: String,
    pub created_at: String,
    #[serde(default)]
    pub token_count: u32,
}

impl SessionContext {
    pub fn to_openai(&self, assistant: &super::peer::Peer) -> Vec<OpenAIMessage> {
        let mut messages = Vec::new();

        if let Some(rep) = &self.peer_representation {
            messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: format!("Peer context for {}: {}", assistant.id, rep),
            });
        }

        if let Some(card) = &self.peer_card {
            if !card.is_empty() {
                let card_str = card.join("\n");
                messages.push(OpenAIMessage {
                    role: "system".to_string(),
                    content: format!("Peer card:\n{}", card_str),
                });
            }
        }

        for msg in &self.messages {
            let role = if msg.peer_id == assistant.id {
                "assistant".to_string()
            } else {
                "user".to_string()
            };
            messages.push(OpenAIMessage {
                role,
                content: msg.content.clone(),
            });
        }

        messages
    }

    pub fn to_anthropic(&self, assistant: &super::peer::Peer) -> Vec<AnthropicMessage> {
        let mut messages = Vec::new();

        if let Some(rep) = &self.peer_representation {
            messages.push(AnthropicMessage {
                role: "assistant".to_string(),
                content: format!("Peer context for {}: {}", assistant.id, rep),
            });
        }

        if let Some(card) = &self.peer_card {
            if !card.is_empty() {
                let card_str = card.join("\n");
                messages.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: format!("Peer card:\n{}", card_str),
                });
            }
        }

        for msg in &self.messages {
            let role = if msg.peer_id == assistant.id {
                "assistant".to_string()
            } else {
                "user".to_string()
            };
            messages.push(AnthropicMessage {
                role,
                content: msg.content.clone(),
            });
        }

        messages
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}
