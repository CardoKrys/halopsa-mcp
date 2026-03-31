use sha2::{Digest, Sha256};

/// Current chunk version. Bump to trigger full re-index.
pub const CHUNK_VERSION: u32 = 1;

const MAX_CHUNK_SIZE: usize = 1200;
const MIN_CHUNK_SIZE: usize = 50;
const OVERLAP_SIZE: usize = 150;

/// A chunk of ticket content ready for embedding.
#[derive(Debug)]
pub struct Chunk {
    pub content: String,
    pub heading_path: String,
    pub content_hash: String,
}

/// Chunk a ticket into pieces suitable for embedding.
/// Produces chunks from the ticket's summary, details, actions, and metadata.
pub fn chunk_ticket(
    ticket_id: i64,
    summary: &str,
    details: &str,
    actions: &[ActionChunkInput],
    metadata: &TicketMetadataInput,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();

    // Main ticket chunk: summary + metadata context + details
    let mut main_content = format!("Ticket #{ticket_id}: {summary}\n");

    // Add metadata context
    if let Some(ref client) = metadata.client_name {
        main_content.push_str(&format!("Client: {client}\n"));
    }
    if let Some(ref site) = metadata.site_name {
        main_content.push_str(&format!("Site: {site}\n"));
    }
    if let Some(ref agent) = metadata.agent_name {
        main_content.push_str(&format!("Agent: {agent}\n"));
    }
    if let Some(ref team) = metadata.team {
        main_content.push_str(&format!("Team/Queue: {team}\n"));
    }
    if let Some(ref status) = metadata.status {
        main_content.push_str(&format!("Status: {status}\n"));
    }
    if let Some(ref priority) = metadata.priority {
        main_content.push_str(&format!("Priority: {priority}\n"));
    }
    if let Some(ref category) = metadata.category {
        main_content.push_str(&format!("Category: {category}\n"));
    }
    if !metadata.custom_fields.is_empty() {
        for (name, value) in &metadata.custom_fields {
            main_content.push_str(&format!("{name}: {value}\n"));
        }
    }
    if !metadata.assets.is_empty() {
        main_content.push_str(&format!("Assets: {}\n", metadata.assets.join(", ")));
    }

    main_content.push('\n');

    // Add details (strip HTML tags for embedding)
    let clean_details = strip_html(details);
    if !clean_details.is_empty() {
        main_content.push_str(&clean_details);
    }

    // Split main content into chunks if too large
    let heading = format!("Ticket #{ticket_id}");
    split_and_add(&main_content, &heading, &mut chunks);

    // Action chunks: each significant action becomes its own chunk
    for action in actions {
        let clean_note = strip_html(&action.note);
        if clean_note.len() < MIN_CHUNK_SIZE {
            continue; // Skip trivial actions
        }

        let mut action_content = format!(
            "Ticket #{ticket_id} > Action by {}\n",
            action.who.as_deref().unwrap_or("Unknown")
        );
        if let Some(ref outcome) = action.outcome {
            action_content.push_str(&format!("Outcome: {outcome}\n"));
        }
        if let Some(ref date) = action.date {
            action_content.push_str(&format!("Date: {date}\n"));
        }
        action_content.push('\n');
        action_content.push_str(&clean_note);

        let action_heading = format!("Ticket #{ticket_id} > Action");
        split_and_add(&action_content, &action_heading, &mut chunks);
    }

    chunks
}

fn split_and_add(content: &str, heading: &str, chunks: &mut Vec<Chunk>) {
    if content.len() <= MAX_CHUNK_SIZE {
        let hash = sha256_hex(content);
        chunks.push(Chunk {
            content: content.to_string(),
            heading_path: heading.to_string(),
            content_hash: hash,
        });
        return;
    }

    // Split by paragraphs first
    let paragraphs: Vec<&str> = content.split("\n\n").collect();
    let mut current = String::new();
    let mut part = 1;

    for para in paragraphs {
        if current.len() + para.len() + 2 > MAX_CHUNK_SIZE && !current.is_empty() {
            let hash = sha256_hex(&current);
            chunks.push(Chunk {
                content: current.clone(),
                heading_path: format!("{heading} (part {part})"),
                content_hash: hash,
            });
            part += 1;

            // Overlap: keep last OVERLAP_SIZE chars
            if current.len() > OVERLAP_SIZE {
                current = current[current.len() - OVERLAP_SIZE..].to_string();
            }
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(para);
    }

    if !current.is_empty() && current.len() >= MIN_CHUNK_SIZE {
        let hash = sha256_hex(&current);
        chunks.push(Chunk {
            content: current,
            heading_path: if part > 1 {
                format!("{heading} (part {part})")
            } else {
                heading.to_string()
            },
            content_hash: hash,
        });
    }
}

/// Input for chunking an action.
pub struct ActionChunkInput {
    pub note: String,
    pub who: Option<String>,
    pub outcome: Option<String>,
    pub date: Option<String>,
}

/// Metadata context for a ticket (used in chunk prefix).
pub struct TicketMetadataInput {
    pub client_name: Option<String>,
    pub site_name: Option<String>,
    pub agent_name: Option<String>,
    pub team: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub category: Option<String>,
    pub custom_fields: Vec<(String, String)>,
    pub assets: Vec<String>,
}

/// Compute the SHA-256 hex digest of a string.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Strip HTML tags from a string, producing plain text.
pub fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut _last_was_block = false;

    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
            }
            '>' => {
                in_tag = false;
                _last_was_block = true;
            }
            _ if !in_tag => {
                _last_was_block = false;
                result.push(ch);
            }
            _ => {}
        }
    }

    // Decode common HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        // Collapse whitespace
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}
