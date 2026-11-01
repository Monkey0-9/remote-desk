use std::collections::HashMap;

pub const CHUNK_MTU: usize = 1200;
pub const CHUNK_HEADER_SIZE: usize = 12; // 4 bytes seq + 2 bytes index + 2 bytes total + 4 bytes reserved

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketChunk {
    pub sequence: u32,
    pub index: u16,
    pub total: u16,
    pub data: Vec<u8>,
}

impl PacketChunk {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(CHUNK_HEADER_SIZE + self.data.len());
        buf.extend_from_slice(&self.sequence.to_be_bytes());
        buf.extend_from_slice(&self.index.to_be_bytes());
        buf.extend_from_slice(&self.total.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // Reserved
        buf.extend_from_slice(&self.data);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < CHUNK_HEADER_SIZE {
            return None;
        }

        let sequence = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        let index = u16::from_be_bytes(bytes[4..6].try_into().ok()?);
        let total = u16::from_be_bytes(bytes[6..8].try_into().ok()?);
        let data = bytes[CHUNK_HEADER_SIZE..].to_vec();

        Some(Self {
            sequence,
            index,
            total,
            data,
        })
    }
}

/// Splits a large payload into chunks suitable for MTU-bounded UDP/DataChannels
pub fn chunk_payload(sequence: u32, payload: &[u8]) -> Vec<PacketChunk> {
    if payload.is_empty() {
        return vec![PacketChunk {
            sequence,
            index: 0,
            total: 1,
            data: Vec::new(),
        }];
    }

    let chunks: Vec<&[u8]> = payload.chunks(CHUNK_MTU).collect();
    let total = chunks.len() as u16;

    chunks
        .into_iter()
        .enumerate()
        .map(|(i, slice)| PacketChunk {
            sequence,
            index: i as u16,
            total,
            data: slice.to_vec(),
        })
        .collect()
}

/// Reassembles chunks into full payload once all chunks for a sequence arrive
pub struct ChunkReassembler {
    pending: HashMap<u32, (u16, HashMap<u16, Vec<u8>>)>,
}

impl ChunkReassembler {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Ingests a chunk. If this chunk completes the sequence, returns Some(complete_payload).
    pub fn ingest_chunk(&mut self, chunk: PacketChunk) -> Option<Vec<u8>> {
        let entry = self
            .pending
            .entry(chunk.sequence)
            .or_insert_with(|| (chunk.total, HashMap::new()));

        entry.1.insert(chunk.index, chunk.data);

        if entry.1.len() as u16 == entry.0 {
            // Reassembly complete
            let (total, mut map) = self.pending.remove(&chunk.sequence).unwrap();
            let mut assembled = Vec::new();
            for i in 0..total {
                let part = map.remove(&i)?;
                assembled.extend_from_slice(&part);
            }
            Some(assembled)
        } else {
            None
        }
    }

    /// Prune old sequences to avoid memory leak if packets were dropped
    pub fn prune_older_than(&mut self, min_sequence: u32) {
        self.pending.retain(|&seq, _| seq >= min_sequence);
    }
}

impl Default for ChunkReassembler {
    fn default() -> Self {
        Self::new()
    }
}
