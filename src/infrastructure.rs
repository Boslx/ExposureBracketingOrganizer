use crate::domain::models::{ExposureInfo, FileMetadata};
use crate::domain::ports::MetadataExtractor;
use num_rational::Rational32;
use rawler::decoders::{RawDecodeParams, RawMetadata};
use rawler::{get_decoder, rawsource::RawSource};
use std::path::Path;

pub struct RawlerMetadataExtractor;

impl RawlerMetadataExtractor {
    pub fn new() -> Self {
        Self
    }

    fn extract_raw(&self, path: &Path) -> Option<RawMetadata> {
        let raw_file = RawSource::new(path).ok()?;
        let decoder = get_decoder(&raw_file).ok()?;
        decoder
            .raw_metadata(&raw_file, &RawDecodeParams::default())
            .ok()
    }
}

impl MetadataExtractor for RawlerMetadataExtractor {
    fn extract(&self, path: &Path) -> Option<ExposureInfo> {
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if let Some(raw_metadata) = self.extract_raw(path) {
            let exposure_bias = raw_metadata
                .exif
                .exposure_bias
                .map(|eb| Rational32::new(eb.n, eb.d));
            let exposure_mode = raw_metadata.exif.exposure_mode;
            Some(ExposureInfo {
                filename,
                exposure_bias_n: exposure_bias.map(|eb| *eb.numer()),
                exposure_bias_d: exposure_bias.map(|eb| *eb.denom()),
                exposure_mode,
                error_message: if exposure_bias.is_none() {
                    Some("No exposure bias found".to_string())
                } else {
                    None
                },
            })
        } else {
            Some(ExposureInfo {
                filename,
                exposure_bias_n: None,
                exposure_bias_d: None,
                exposure_mode: None,
                error_message: Some("Could not read metadata".to_string()),
            })
        }
    }

    fn extract_metadata(&self, path: &Path) -> Option<FileMetadata> {
        if let Some(raw_metadata) = self.extract_raw(path) {
            let exposure_bias = raw_metadata
                .exif
                .exposure_bias
                .map(|eb| Rational32::new(eb.n, eb.d));
            let exposure_mode = raw_metadata.exif.exposure_mode;

            Some(FileMetadata {
                path: path.to_path_buf(),
                exposure_bias,
                exposure_mode,
            })
        } else {
            None
        }
    }
}
