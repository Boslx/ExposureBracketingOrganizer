use num_rational::Rational32;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    MoveToFolder,
    SaveSequencesToTextfile,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvMode {
    Absolute,
    Delta,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BracketOrder {
    ZeroMinusPlus,
    MinusZeroPlus,
}

impl std::fmt::Display for BracketOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BracketOrder::ZeroMinusPlus => write!(f, "ZeroMinusPlus"),
            BracketOrder::MinusZeroPlus => write!(f, "MinusZeroPlus"),
        }
    }
}

impl std::fmt::Display for EvMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvMode::Absolute => write!(f, "Absolute EV Value"),
            EvMode::Delta => write!(f, "Delta EV Change"),
        }
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::MoveToFolder => write!(f, "Move to Folder"),
            Action::SaveSequencesToTextfile => write!(f, "Save Sequences to Textfile"),
        }
    }
}

#[derive(Debug)]
pub struct ExposureInfo {
    pub filename: String,
    pub exposure_bias_n: Option<i32>,
    pub exposure_bias_d: Option<i32>,
    pub exposure_mode: Option<u16>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExposureSettings {
    pub ev_step: f32,
    pub num_images: u32,
    pub bracket_order: BracketOrder,
}

impl Default for ExposureSettings {
    fn default() -> Self {
        Self {
            ev_step: 1.0,
            num_images: 3,
            bracket_order: BracketOrder::ZeroMinusPlus,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub exposure_bias: Option<Rational32>,
    pub exposure_mode: Option<u16>,
}
