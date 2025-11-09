use crate::app::{Action, EvMode};
use chrono::{DateTime, Local};
use log::{debug, info, warn};
use num_rational::Rational32;
use num_traits::Zero;
use rawler::decoders::{RawDecodeParams, RawMetadata};
use rawler::{get_decoder, rawsource::RawSource};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub fn count_files_in_directory(dir: &Path, extensions: &Vec<String>) -> usize {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    entries
        .flatten()
        .filter(|e| {
            let path = e.path();
            if !path.is_file() {
                return false;
            }
            path.extension()
                .and_then(|s| s.to_str())
                .map(|s| extensions.iter().any(|ext| ext.eq_ignore_ascii_case(s)))
                .unwrap_or(false)
        })
        .count()
}

pub fn extract_raw_metadata(path: &Path) -> Option<RawMetadata> {
    let path_str = path.display().to_string();
    log::debug!("Processing file: {}", path_str);

    let raw_file = RawSource::new(path.as_ref()).ok()?;
    let decoder = get_decoder(&raw_file).ok()?;
    decoder
        .raw_metadata(&raw_file, &RawDecodeParams::default())
        .ok()
}

struct FileMetadata {
    path: PathBuf,
    //creation_time: DateTime<Local>,
    exposure_bias: Option<Rational32>,
    exposure_mode: Option<u16>,
}

pub fn process_directory(
    dir: &Path,
    processed_files: &Arc<AtomicUsize>,
    exposure_bracketings_found: &Arc<AtomicUsize>,
    extensions: Vec<String>,
    sequence: Vec<Rational32>,
    selected_action: Action,
    ev_mode: EvMode,
    filter_by_auto_bracket: bool,
) {
    let files_with_metadata =
        collect_files_with_metadata(dir, processed_files, &extensions, filter_by_auto_bracket);

    // Just relying on the order in the filesystem is good enough
    // A timestamp can be ambiguous as well
    //files_with_metadata.sort_by_key(|f| f.creation_time);

    let matching_sequences = find_matching_sequences(&files_with_metadata, &sequence, ev_mode);

    for seq in matching_sequences {
        exposure_bracketings_found.fetch_add(1, Ordering::Relaxed);
        execute_action_on_sequence(dir, seq, selected_action.clone());
    }
}

fn collect_files_with_metadata(
    dir: &Path,
    processed_files: &Arc<AtomicUsize>,
    extensions: &Vec<String>,
    filter_by_auto_bracket: bool,
) -> Vec<FileMetadata> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read directory {}: {}", dir.display(), e);
            return Vec::new();
        }
    };

    let mut files_with_metadata: Vec<FileMetadata> = Vec::new();

    for entry in entries.flatten() {
        processed_files.fetch_add(1, Ordering::Relaxed);
        let path = entry.path();
        if path.is_file() {
            let ext_match = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .map(|s| extensions.iter().any(|pat| pat == &s))
                .unwrap_or(false);

            if ext_match {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(created) = metadata.created() {
                        let datetime: DateTime<Local> = created.into();
                        if let Some(raw_metadata) = extract_raw_metadata(&path) {
                            let exposure_bias = raw_metadata
                                .exif
                                .exposure_bias
                                .map(|eb| Rational32::new(eb.n, eb.d));
                            let exposure_mode = raw_metadata.exif.exposure_mode;

                            if filter_by_auto_bracket {
                                if let Some(mode) = exposure_mode {
                                    if mode != 2 {
                                        continue;
                                    }
                                } else {
                                    continue;
                                }
                            }

                            files_with_metadata.push(FileMetadata {
                                path: path.clone(),
                                //creation_time: datetime,
                                exposure_bias,
                                exposure_mode,
                            });
                        }
                    }
                }
            }
        }
    }
    files_with_metadata
}

fn find_matching_sequences<'a>(
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
                        warn!(
                            "Delta EV mode requires a 0.0 value in the sequence to act as a reference."
                        );
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
                            debug!(
                                "Current bias: {}, Base bias: {}, Seq delta: {}",
                                current_bias, base_bias, seq_delta
                            );
                            let delta = current_bias - base_bias;
                            debug!("Calculated delta: {}", delta);
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

fn execute_action_on_sequence(dir: &Path, sequence: &[FileMetadata], action: Action) {
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
                        if let Err(e) = fs::rename(&file_meta.path, new_file_path) {
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
            let file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path);

            match file {
                Ok(mut f) => {
                    for file_meta in sequence {
                        if let Err(e) = writeln!(f, "{}", file_meta.path.display()) {
                            warn!("Failed to write to sequences.txt: {}", e);
                        }
                    }
                    if let Err(e) = writeln!(f) {
                        // Add a blank line between sequences
                        warn!("Failed to write to sequences.txt: {}", e);
                    }
                    info!("Appended sequence to {}", "sequences.txt");
                }
                Err(e) => {
                    warn!("Failed to open sequences.txt: {}", e);
                }
            }
        }
    }
}
