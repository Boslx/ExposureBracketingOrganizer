use crate::domain::models::{ExposureInfo, FileMetadata};
use std::path::Path;

pub trait MetadataExtractor {
    fn extract(&self, path: &Path) -> Option<ExposureInfo>;
    fn extract_metadata(&self, path: &Path) -> Option<FileMetadata>;
}
