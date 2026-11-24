//! Midi Fighter Twister configuration SysEx protocol.
//!
//! Recovered from Midi Fighter Utility (macOS, arm64) with REA + objdump; see
//! `research/PROTOCOL.md`. All messages are
//! `F0 00 01 79 <payload…> F7` (DJTT manufacturer id 00 01 79), every payload
//! byte masked to 7 bits.
//!
//! | Request                | Payload                                            |
//! |------------------------|----------------------------------------------------|
//! | pull global config     | `02 00`                                            |
//! | global config reply    | `02 01 (tag value)*`                               |
//! | push global config     | `01 (tag value)*`                                  |
//! | pull encoder record    | `04 01 <record>`                                   |
//! | encoder record chunk   | `04 00 <record> <part> <total> <len> <data…>`      |
//!
//! Encoder records are numbered from 1 (bank * 16 + encoder + 1) and hold
//! 16 `(tag, value)` pairs; they are split into chunks of at most 24 bytes.

pub const MANUFACTURER: [u8; 3] = [0x00, 0x01, 0x79];
pub const BULK_CHUNK: usize = 24;

const CMD_PUSH_CONFIG: u8 = 0x01;
const CMD_PULL_CONFIG: u8 = 0x02;
const CMD_BULK: u8 = 0x04;
const BULK_PUSH: u8 = 0x00;
const BULK_PULL: u8 = 0x01;

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(payload.len() + 5);
    msg.push(0xF0);
    msg.extend_from_slice(&MANUFACTURER);
    msg.extend(payload.iter().map(|b| b & 0x7F));
    msg.push(0xF7);
    msg
}

pub fn pull_globals() -> Vec<u8> {
    frame(&[CMD_PULL_CONFIG, 0x00])
}

pub fn push_globals(pairs: &[(u8, u8)]) -> Vec<u8> {
    let mut payload = vec![CMD_PUSH_CONFIG];
    for &(tag, value) in pairs {
        payload.push(tag);
        payload.push(value);
    }
    frame(&payload)
}

pub fn pull_encoder(record: u8) -> Vec<u8> {
    frame(&[CMD_BULK, BULK_PULL, record])
}

/// One SysEx message per chunk of at most [`BULK_CHUNK`] data bytes.
pub fn push_encoder(record: u8, data: &[u8]) -> Vec<Vec<u8>> {
    let total = data.len().div_ceil(BULK_CHUNK);
    data.chunks(BULK_CHUNK)
        .enumerate()
        .map(|(i, chunk)| {
            let mut payload = vec![
                CMD_BULK,
                BULK_PUSH,
                record,
                (i + 1) as u8,
                total as u8,
                chunk.len() as u8,
            ];
            payload.extend_from_slice(chunk);
            frame(&payload)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Globals(Vec<(u8, u8)>),
    EncoderChunk { record: u8, part: u8, total: u8, data: Vec<u8> },
}

/// Parse a complete SysEx message (including F0 … F7) coming from the device.
pub fn parse(msg: &[u8]) -> Option<Reply> {
    let body = msg.strip_prefix(&[0xF0])?;
    let body = body.strip_suffix(&[0xF7]).unwrap_or(body);
    let payload = body.strip_prefix(&MANUFACTURER)?;
    match payload {
        [CMD_PULL_CONFIG, 0x01, rest @ ..] if rest.len() >= 2 => Some(Reply::Globals(
            rest.chunks_exact(2).map(|p| (p[0], p[1])).collect(),
        )),
        [CMD_BULK, BULK_PUSH, record, part, total, _len, data @ ..] => Some(Reply::EncoderChunk {
            record: *record,
            part: *part,
            total: *total,
            data: data.to_vec(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_globals_matches_utility() {
        assert_eq!(pull_globals(), vec![0xF0, 0x00, 0x01, 0x79, 0x02, 0x00, 0xF7]);
    }

    #[test]
    fn pull_encoder_matches_utility() {
        assert_eq!(pull_encoder(5), vec![0xF0, 0x00, 0x01, 0x79, 0x04, 0x01, 0x05, 0xF7]);
    }

    #[test]
    fn push_globals_frames_pairs() {
        assert_eq!(
            push_globals(&[(0, 4), (31, 127)]),
            vec![0xF0, 0x00, 0x01, 0x79, 0x01, 0, 4, 31, 127, 0xF7]
        );
    }

    #[test]
    fn push_encoder_splits_32_bytes_into_24_and_8() {
        let data: Vec<u8> = (0..32).collect();
        let msgs = push_encoder(1, &data);
        assert_eq!(msgs.len(), 2);
        assert_eq!(&msgs[0][..10], &[0xF0, 0x00, 0x01, 0x79, 0x04, 0x00, 1, 1, 2, 24]);
        assert_eq!(msgs[0].len(), 10 + 24 + 1);
        assert_eq!(&msgs[1][..10], &[0xF0, 0x00, 0x01, 0x79, 0x04, 0x00, 1, 2, 2, 8]);
        assert_eq!(*msgs[1].last().unwrap(), 0xF7);
    }

    #[test]
    fn parses_global_reply() {
        let msg = [0xF0, 0x00, 0x01, 0x79, 0x02, 0x01, 0, 4, 1, 1, 0xF7];
        assert_eq!(parse(&msg), Some(Reply::Globals(vec![(0, 4), (1, 1)])));
    }

    #[test]
    fn parses_encoder_chunk_roundtrip() {
        let data: Vec<u8> = (0..32).collect();
        let msgs = push_encoder(7, &data);
        let mut joined = Vec::new();
        for m in &msgs {
            match parse(m) {
                Some(Reply::EncoderChunk { record: 7, data, .. }) => joined.extend(data),
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(joined, data);
    }

    #[test]
    fn ignores_foreign_sysex() {
        assert_eq!(parse(&[0xF0, 0x7E, 0x00, 0x06, 0x02, 0xF7]), None);
    }
}
