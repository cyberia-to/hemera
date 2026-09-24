//! Commit a validated `.cyb` extraction without internal heap allocation.
//! The caller owns input buffers and name scratch space. This is not a parser.
use crate::{
    cdc::{CdcError, chunk_ranges},
    commitment::{self, ContentId, SequenceBuilder, SequenceId},
};

#[derive(Debug, Clone, Copy)]
pub struct Entry<'a> {
    pub name: &'a str,
    pub declaration: &'a [u8],
    pub content: &'a [u8],
    pub element_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidName,
    DuplicateName,
    ScratchTooSmall,
    TooManyItems,
    Elements(CdcError),
}

#[derive(Debug, Clone)]
pub struct SectionCommitment {
    pub id: ContentId,
    pub chunk_count: u64,
    pub sequence: SequenceId,
}

#[derive(Debug, Clone)]
pub struct EntryCommitment {
    pub id: ContentId,
    pub declaration: ContentId,
    pub content: ContentId,
}

#[derive(Debug, Clone)]
pub struct ContainerCommitment {
    pub id: ContentId,
    pub preamble: ContentId,
    pub entry_count: u64,
    pub sequence: SequenceId,
}

/// The same section has the same ID standalone and inside any container.
pub fn section(data: &[u8], element_size: usize) -> Result<SectionCommitment, Error> {
    let ranges = chunk_ranges(data, element_size).map_err(Error::Elements)?;
    let mut builder = SequenceBuilder::default();
    for range in ranges {
        builder
            .push(commitment::blob(&data[range]))
            .map_err(|_| Error::TooManyItems)?;
    }
    let sequence = builder.finish();
    let id = commitment::record(
        b"cyb.section.v1",
        &[
            commitment::blob(&(element_size as u64).to_le_bytes()),
            commitment::blob(&(data.len() as u64).to_le_bytes()),
            commitment::blob(sequence.as_bytes()),
        ],
    );
    Ok(SectionCommitment {
        id,
        chunk_count: builder.count(),
        sequence,
    })
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.bytes().any(|b| b == b'\n' || b == b'\r')
}

/// Commit one entry independently of its position in a container.
pub fn entry(entry: &Entry<'_>) -> Result<EntryCommitment, Error> {
    if !valid_name(entry.name) {
        return Err(Error::InvalidName);
    }
    let declaration = section(entry.declaration, 1)?.id;
    let content = section(entry.content, entry.element_size)?.id;
    let id = commitment::record(
        b"cyb.entry.v1",
        &[
            commitment::blob(entry.name.as_bytes()),
            declaration,
            content,
        ],
    );
    Ok(EntryCommitment {
        id,
        declaration,
        content,
    })
}

/// The caller validates correspondence with parsed source. Scratch must hold
/// at least entries.len() names; its contents are overwritten and sorted.
pub fn container<'a>(
    preamble: &[u8],
    entries: &[Entry<'a>],
    scratch: &mut [&'a str],
) -> Result<ContainerCommitment, Error> {
    if scratch.len() < entries.len() {
        return Err(Error::ScratchTooSmall);
    }
    let names = &mut scratch[..entries.len()];
    for (slot, entry) in names.iter_mut().zip(entries) {
        if !valid_name(entry.name) {
            return Err(Error::InvalidName);
        }
        *slot = entry.name;
        if !(1..=64).contains(&entry.element_size) {
            return Err(Error::Elements(CdcError::ElementSize));
        }
        if !entry.content.len().is_multiple_of(entry.element_size) {
            return Err(Error::Elements(CdcError::MisalignedLength));
        }
    }
    names.sort_unstable();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::DuplicateName);
    }
    let preamble = section(preamble, 1)?.id;
    let mut builder = SequenceBuilder::default();
    for item in entries {
        builder
            .push(entry(item)?.id)
            .map_err(|_| Error::TooManyItems)?;
    }
    let sequence = builder.finish();
    let id = commitment::record(
        b"cyb.container.v1",
        &[preamble, commitment::blob(sequence.as_bytes())],
    );
    Ok(ContainerCommitment {
        id,
        preamble,
        entry_count: builder.count(),
        sequence,
    })
}
