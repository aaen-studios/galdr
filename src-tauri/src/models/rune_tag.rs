use serde::{Deserialize, Serialize};

use crate::models::ConversionParams;

/// A rune tag captures a conversion preset.
///
/// `PresetParams` is intentionally an alias of `ConversionParams` so that
/// every conversion parameter (current and future) is automatically saveable,
/// persistable, and applyable as a rune — there is only ever one source of
/// truth for "what a conversion looks like". The two job-specific path fields
/// (`input_path`, `output_dir`) are zeroed before a rune is persisted, since
/// they describe a particular file, not a reusable preset.
pub type PresetParams = ConversionParams;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuneTag {
    pub id: String,
    pub name: String,
    pub rune: String,
    pub description: String,
    pub params: PresetParams,
}
