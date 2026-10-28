mod attachment_store;
mod code_highlight;
mod diagram_cache;
mod diagram_element;
mod embed_image;
mod embed_resolution;
mod file_card;
mod generations;
mod link_resolution;
mod vault_files;
mod vault_index;
mod vault_paths;

pub use attachment_store::{AttachmentFailure, AttachmentSource, store_attachments};
pub use code_highlight::highlight_code;
pub use diagram_cache::{DiagramCache, DiagramImage, DiagramState, math, mermaid};
pub use diagram_element::{DiagramKind, diagram_element};
pub use embed_image::{EmbedImage, cached_image, embed_image, image_resource};
pub use embed_resolution::{
    Embed, EmbedSize, ImageLocation, MAX_EMBED_DEPTH, ResolvedImage, embed_size, resolve_embed,
    resolve_image,
};
pub use file_card::{EmbedKind, embed_icon, file_card};
pub use link_resolution::{LinkRef, Resolved, resolve_link, web_href};
pub use margin_diagrams::MathMode;
pub use vault_files::{NoteTextState, file_size, heading_section, note_text};
pub use vault_index::VaultIndex;
