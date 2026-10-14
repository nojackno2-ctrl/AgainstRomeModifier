//! ALR v4..6 source header and the metadata normalization used by 0x4e3a20.
//! This is not a frame/pixel parser or proof that an asset is loaded in the game.

use std::fmt;

pub const HEADER_WORDS: usize = 23;
pub const HEADER_BYTES: usize = HEADER_WORDS * 4;
/// Defensive parser limits, not observed native validation rules.
pub const MAX_FRAME_COUNT: u32 = 100_000;
pub const MAX_PALETTE_VARIANTS: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataError {
    TruncatedHeader {
        available: usize,
    },
    InvalidSignature(u32),
    UnsupportedVersion(u32),
    UnsupportedDepth(u32),
    InvalidFrameCount(u32),
    InvalidPaletteVariants(u32),
    /// Even one reference word per frame cannot fit; full payload validation is out of scope.
    TruncatedFrameReferences {
        required: u64,
        available: u64,
    },
    LayoutProductOverflow {
        a: u32,
        b: u32,
    },
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ALR metadata error: {self:?}")
    }
}
impl std::error::Error for MetadataError {}

/// Immutable serialized values, with only header/count extent validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceHeaderMetadata {
    words: [u32; HEADER_WORDS],
}

impl SourceHeaderMetadata {
    /// Reads exactly 23 little-endian words. Does not inspect frame references or pixels.
    pub fn parse(bytes: &[u8]) -> Result<Self, MetadataError> {
        let header = bytes
            .get(..HEADER_BYTES)
            .ok_or(MetadataError::TruncatedHeader {
                available: bytes.len(),
            })?;
        let mut words = [0; HEADER_WORDS];
        for (word, chunk) in words.iter_mut().zip(header.chunks_exact(4)) {
            *word = u32::from_le_bytes(chunk.try_into().expect("four-byte chunk"));
        }
        Self::from_header_words(words, bytes.len() as u64)
    }

    /// For an existing inventory: `source_len` is the serialized ALR member length.
    /// Caller provenance is not authenticated here; words/length must come from the same member.
    pub fn from_header_words(
        words: [u32; HEADER_WORDS],
        source_len: u64,
    ) -> Result<Self, MetadataError> {
        if source_len < HEADER_BYTES as u64 {
            return Err(MetadataError::TruncatedHeader {
                available: source_len as usize,
            });
        }
        if words[0] != 0x4152_4c41 {
            return Err(MetadataError::InvalidSignature(words[0]));
        }
        if !(4..=6).contains(&words[1]) {
            return Err(MetadataError::UnsupportedVersion(words[1]));
        }
        if words[4] != 8 {
            return Err(MetadataError::UnsupportedDepth(words[4]));
        }
        if words[3] == 0 || words[3] > MAX_FRAME_COUNT {
            return Err(MetadataError::InvalidFrameCount(words[3]));
        }
        if words[8] == 0 || words[8] > MAX_PALETTE_VARIANTS {
            return Err(MetadataError::InvalidPaletteVariants(words[8]));
        }
        let required = HEADER_BYTES as u64 + u64::from(words[3]) * 4;
        if required > source_len {
            return Err(MetadataError::TruncatedFrameReferences {
                required,
                available: source_len,
            });
        }
        Ok(Self { words })
    }

    pub fn raw_words(&self) -> &[u32; HEADER_WORDS] {
        &self.words
    }
    pub fn version(&self) -> u32 {
        self.words[1]
    }
    pub fn frame_count(&self) -> u32 {
        self.words[3]
    }
    pub fn depth(&self) -> u32 {
        self.words[4]
    }
    pub fn raw_columns(&self) -> u32 {
        self.words[6]
    }
    pub fn raw_rows(&self) -> u32 {
        self.words[7]
    }
    pub fn palette_variants(&self) -> u32 {
        self.words[8]
    }

    /// Models runtime words 4/5 only. 0x4e4170 always uses Enabled.
    /// Rejects overflow rather than reproducing unsafe native wrapped layouts.
    pub fn loaded_metadata(
        &self,
        mode: Normalization,
    ) -> Result<LoadedHeaderMetadata, MetadataError> {
        let mut a = self.raw_columns();
        let mut b = self.raw_rows();
        let mut changes = NormalizationChanges::default();
        if mode == Normalization::Enabled {
            changes.zero_a_to_one = a == 0;
            changes.zero_b_to_one = b == 0;
            a = a.max(1);
            b = b.max(1);
        }
        let product = a
            .checked_mul(b)
            .ok_or(MetadataError::LayoutProductOverflow { a, b })?;
        // Native IMUL keeps low 32 bits, CMP/JBE is unsigned. Within the accepted domain
        // this is exactly product >= count && product - count != 0 (i.e. product > count).
        if mode == Normalization::Enabled && product > self.frame_count() {
            a = self.frame_count();
            b = 1;
            changes.collapsed_to_frame_count = true;
        }
        let product = a.checked_mul(b).expect("checked or bounded replacement");
        let groups = if product == 0 {
            None
        } else {
            Some(GroupObservation {
                layout_product: product,
                whole_groups: self.frame_count() / product,
                remainder_frames: self.frame_count() % product,
            })
        };
        Ok(LoadedHeaderMetadata {
            source: self.clone(),
            mode,
            a,
            b,
            changes,
            groups,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Normalization {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NormalizationChanges {
    pub zero_a_to_one: bool,
    pub zero_b_to_one: bool,
    pub collapsed_to_frame_count: bool,
}

/// Arithmetic observation only; does not establish animation/frame eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupObservation {
    pub layout_product: u32,
    pub whole_groups: u32,
    pub remainder_frames: u32,
}

/// Successful header-derived metadata, not a resident native asset or validated frame payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedHeaderMetadata {
    source: SourceHeaderMetadata,
    mode: Normalization,
    a: u32,
    b: u32,
    changes: NormalizationChanges,
    groups: Option<GroupObservation>,
}
impl LoadedHeaderMetadata {
    pub fn source(&self) -> &SourceHeaderMetadata {
        &self.source
    }
    pub fn normalization(&self) -> Normalization {
        self.mode
    }
    /// Runtime +0x10, returned by 0x4af200 for a loaded asset.
    pub fn a(&self) -> u32 {
        self.a
    }
    /// Runtime +0x14, returned by 0x4af230 for a loaded asset.
    pub fn b(&self) -> u32 {
        self.b
    }
    pub fn changes(&self) -> NormalizationChanges {
        self.changes
    }
    /// None means a zero product with normalization disabled, not zero groups.
    pub fn groups(&self) -> Option<GroupObservation> {
        self.groups
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnloadedReason {
    InvalidId,
    NoLoadedMetadata,
}

/// Keeps native default observations separate from successfully parsed metadata.
/// `loaded` is supplied by the caller; this function never loads or resolves an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeQueryObservation<'a> {
    HeaderMetadata(&'a LoadedHeaderMetadata),
    UnloadedDefault {
        reason: UnloadedReason,
        a: u32,
        b: u32,
    },
}
pub fn observe_native_query(
    id: i32,
    loaded: Option<&LoadedHeaderMetadata>,
) -> NativeQueryObservation<'_> {
    let reason = if !(0..3000).contains(&id) {
        UnloadedReason::InvalidId
    } else if let Some(metadata) = loaded {
        return NativeQueryObservation::HeaderMetadata(metadata);
    } else {
        UnloadedReason::NoLoadedMetadata
    };
    NativeQueryObservation::UnloadedDefault { reason, a: 1, b: 1 }
}
