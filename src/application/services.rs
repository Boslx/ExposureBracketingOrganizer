use crate::domain::models::{Action, EvMode, FileMetadata};
use crate::domain::ports::MetadataExtractor;
use log::{info, warn};
use num_rational::Rational32;
use num_traits::Zero;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct OrganizerService<M>
where
    M: MetadataExtractor,
{
    metadata_extractor: M,
}

impl<M> OrganizerService<M>
where
    M: MetadataExtractor,
{
    pub fn new(metadata_extractor: M) -> Self {
        Self { metadata_extractor }
    }

    pub fn process_directory(
        &self,
        dir: &Path,
        processed_files: &Arc<AtomicUsize>,
        exposure_bracketings_found: &Arc<AtomicUsize>,
        extensions: &[String],
        sequence: &[Rational32],
        selected_action: Action,
        ev_mode: EvMode,
        filter_by_auto_bracket: bool,
    ) {
        let files_with_metadata = self.collect_files_with_metadata(
            dir,
            processed_files,
            extensions,
            filter_by_auto_bracket,
        );

        let matching_sequences =
            self.find_matching_sequences(&files_with_metadata, sequence, ev_mode);

        for seq in matching_sequences {
            exposure_bracketings_found.fetch_add(1, Ordering::Relaxed);
            self.execute_action_on_sequence(dir, seq, &selected_action);
        }
    }

    fn collect_files_with_metadata(
        &self,
        dir: &Path,
        processed_files: &Arc<AtomicUsize>,
        extensions: &[String],
        filter_by_auto_bracket: bool,
    ) -> Vec<FileMetadata> {
        let paths = self.list_files(dir, extensions);
        let mut files_with_metadata = Vec::new();

        for path in paths {
            processed_files.fetch_add(1, Ordering::Relaxed);
            if let Some(metadata) = self.metadata_extractor.extract_metadata(&path) {
                if filter_by_auto_bracket {
                    if let Some(mode) = metadata.exposure_mode {
                        if mode != 2 {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                files_with_metadata.push(metadata);
            }
        }
        files_with_metadata
    }

    fn list_files(&self, dir: &Path, extensions: &[String]) -> Vec<PathBuf> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        let mut paths: Vec<PathBuf> = entries
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                if !path.is_file() {
                    return None;
                }
                let ext_match = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| extensions.iter().any(|ext| ext.eq_ignore_ascii_case(s)))
                    .unwrap_or(false);

                if ext_match {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        paths.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
        paths
    }

    fn find_matching_sequences<'a>(
        &self,
        files: &'a [FileMetadata],
        sequence: &[Rational32],
        ev_mode: EvMode,
    ) -> Vec<&'a [FileMetadata]> {
        let sequence_len = sequence.len();
        if sequence_len == 0 {
            warn!("Sequence length is zero, cannot process.");
            return Vec::new();
        }

        if files.len() < sequence_len {
            return Vec::new();
        }

        let mut matching_sequences = Vec::new();

        for file_group in files.windows(sequence_len) {
            let sequence_match = match ev_mode {
                EvMode::Absolute => {
                    file_group
                        .iter()
                        .zip(sequence.iter())
                        .all(|(file_meta, seq_abs)| {
                            if let Some(current_bias) = file_meta.exposure_bias {
                                current_bias == *seq_abs
                            } else {
                                false
                            }
                        })
                }
                EvMode::Delta => {
                    let zero_bias_index = match sequence.iter().position(|r| r.is_zero()) {
                        Some(i) => i,
                        None => {
                            warn!("Delta EV mode requires a 0.0 value in the sequence to act as a reference.");
                            continue;
                        }
                    };

                    let base_bias = match file_group
                        .get(zero_bias_index)
                        .and_then(|f| f.exposure_bias)
                    {
                        Some(b) => b,
                        None => continue,
                    };

                    file_group
                        .iter()
                        .zip(sequence.iter())
                        .all(|(file_meta, seq_delta)| {
                            if let Some(current_bias) = file_meta.exposure_bias {
                                let delta = current_bias - base_bias;
                                delta == *seq_delta
                            } else {
                                false
                            }
                        })
                }
            };

            if sequence_match {
                matching_sequences.push(file_group);
            }
        }
        matching_sequences
    }

    fn execute_action_on_sequence(&self, dir: &Path, sequence: &[FileMetadata], action: &Action) {
        match action {
            Action::MoveToFolder => {
                if let Some(first_file) = sequence.first() {
                    let folder_name = first_file
                        .path
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    let new_folder_path = dir.join(&folder_name);

                    if fs::create_dir(&new_folder_path).is_ok() {
                        for file_meta in sequence {
                            let new_file_path =
                                new_folder_path.join(file_meta.path.file_name().unwrap());
                            if let Err(e) = fs::rename(&file_meta.path, &new_file_path) {
                                warn!(
                                    "Failed to move file {} to {}: {}",
                                    file_meta.path.display(),
                                    folder_name,
                                    e
                                );
                            }
                        }
                        info!("Moved sequence to folder {}", folder_name);
                    } else {
                        warn!("Failed to create folder {}", folder_name);
                    }
                }
            }
            Action::SaveSequencesToTextfile => {
                let file_path = dir.join("sequences.txt");

                let mut file = match fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&file_path)
                {
                    Ok(f) => f,
                    Err(e) => {
                        warn!("Failed to open sequences.txt: {}", e);
                        return;
                    }
                };

                for file_meta in sequence {
                    if let Err(e) = writeln!(file, "{}", file_meta.path.display()) {
                        warn!("Failed to write to sequences.txt: {}", e);
                    }
                }
                if let Err(e) = writeln!(file, "") {
                    warn!("Failed to write to sequences.txt: {}", e);
                }
                info!("Wrote sequence to sequences.txt");
            }
        }
    }

    pub fn count_files(&self, dir: &Path, extensions: &[String]) -> usize {
        self.list_files(dir, extensions).len()
    }

    pub fn extract_exposure_info(
        &self,
        path: &Path,
    ) -> Option<crate::domain::models::ExposureInfo> {
        self.metadata_extractor.extract(path)
    }
}
