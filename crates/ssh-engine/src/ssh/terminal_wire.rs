//! Shared framing for confirmed input on an exclusively claimed terminal.
use base64::{engine::general_purpose::STANDARD, Engine};

// Both codecs produce at most 240 ASCII bytes per D line (242 with D and LF).
pub(super) const INPUT_CHUNK_BYTES: usize = 180;

#[derive(Clone, Copy)]
pub(super) enum InputEncoding {
    Base64,
    Octal,
}

impl InputEncoding {
    /// Only the matching ACK confirms the bytes consumed by the child.
    pub(super) fn confirmed_payload<'a>(
        self,
        command: &'a str,
        nonce: &'a str,
    ) -> impl Iterator<Item = (Vec<u8>, String)> + 'a {
        let (chunk_size, encoded_len) = match self {
            Self::Base64 => (INPUT_CHUNK_BYTES, command.len().div_ceil(3) * 4),
            Self::Octal => (48, command.len() * 5),
        };
        let chunks = command.as_bytes().chunks(chunk_size);
        let total = format!("{}:{encoded_len}", chunks.len());
        let end = (
            format!("E:{total}\n").into_bytes(),
            format!("{nonce}:INPUT_END:{total}"),
        );
        chunks
            .enumerate()
            .map(move |(index, chunk)| {
                use std::fmt::Write;
                let mut line = String::from("D");
                match self {
                    Self::Base64 => STANDARD.encode_string(chunk, &mut line),
                    Self::Octal => {
                        for byte in chunk {
                            write!(line, "\\0{byte:03o}").unwrap();
                        }
                    }
                }
                let part = &line[1..];
                let ack = format!("{nonce}:INPUT:{}:{}:{part}", index + 1, part.len());
                line.push('\n');
                (line.into_bytes(), ack)
            })
            .chain(std::iter::once(end))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Fragment {
    Data(Vec<u8>),
    Frame(String),
}

#[derive(Default)]
pub(super) struct Parser {
    pending: Vec<u8>,
}
impl Parser {
    pub(super) fn push(&mut self, bytes: &[u8]) -> Vec<Fragment> {
        const MAX_PENDING_FRAME_BYTES: usize = 512;
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::new();
        let mut data_start = 0;
        let mut frame_start = None;
        for (i, byte) in self.pending.iter().enumerate() {
            match byte {
                // A frame cannot contain RS. A newer RS makes the preceding
                // unterminated candidate ordinary output, even across packets.
                0x1e => frame_start = Some(i),
                0x1f => {
                    let Some(start) = frame_start
                        .take()
                        .filter(|start| i - start <= MAX_PENDING_FRAME_BYTES)
                    else {
                        continue;
                    };
                    let Ok(frame) = std::str::from_utf8(&self.pending[start + 1..i]) else {
                        continue;
                    };
                    if data_start < start {
                        out.push(Fragment::Data(self.pending[data_start..start].to_vec()));
                    }
                    out.push(Fragment::Frame(frame.to_owned()));
                    data_start = i + 1;
                }
                _ => {}
            }
        }
        let keep_from = frame_start
            .filter(|start| self.pending.len() - start <= MAX_PENDING_FRAME_BYTES)
            .unwrap_or(self.pending.len());
        if data_start < keep_from {
            out.push(Fragment::Data(self.pending[data_start..keep_from].to_vec()));
        }
        self.pending.drain(..keep_from);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payload_lines_stay_below_canonical_line_limits() {
        for codec in [InputEncoding::Base64, InputEncoding::Octal] {
            for command in [
                "".to_owned(),
                "x".repeat(180),
                "x".repeat(181),
                "secret中文\r\n\x03\x1e'\"\\".repeat(1000),
            ] {
                let mut lines = codec.confirmed_payload(&command, "op").collect::<Vec<_>>();
                let (end, end_ack) = lines.pop().unwrap();
                let mut encoded = Vec::new();
                for (i, (line, ack)) in lines.iter().enumerate() {
                    let part = std::str::from_utf8(&line[1..line.len() - 1]).unwrap();
                    assert_eq!(*ack, format!("op:INPUT:{}:{}:{part}", i + 1, part.len()));
                    assert!(line.len() <= 242 && line.is_ascii());
                    assert_eq!(line.first(), Some(&b'D'));
                    assert_eq!(line.last(), Some(&b'\n'));
                    encoded.extend_from_slice(&line[1..line.len() - 1]);
                }
                assert_eq!(
                    end,
                    format!("E:{}:{}\n", lines.len(), encoded.len()).as_bytes()
                );
                assert_eq!(
                    end_ack,
                    format!("op:INPUT_END:{}:{}", lines.len(), encoded.len())
                );
                let decoded = match codec {
                    InputEncoding::Base64 => STANDARD.decode(encoded).unwrap(),
                    InputEncoding::Octal => encoded
                        .chunks_exact(5)
                        .map(|part| {
                            assert_eq!(&part[..2], b"\\0");
                            u8::from_str_radix(std::str::from_utf8(&part[2..]).unwrap(), 8).unwrap()
                        })
                        .collect(),
                };
                assert_eq!(decoded, command.as_bytes());
            }
        }
    }
    #[test]
    fn frames_accept_fragmented_input_and_bound_unterminated_markers() {
        let wire = b"\x1en:BEGIN\x1fYWJj\r\n\x1en:END:0\x1f";
        for split in 0..=wire.len() {
            let mut parser = Parser::default();
            let parts = parser
                .push(&wire[..split])
                .into_iter()
                .chain(parser.push(&wire[split..]))
                .collect::<Vec<_>>();
            assert_eq!(
                parts
                    .iter()
                    .filter_map(|part| if let Fragment::Frame(v) = part {
                        Some(v.as_str())
                    } else {
                        None
                    })
                    .collect::<Vec<_>>(),
                ["n:BEGIN", "n:END:0"]
            );
        }
        let mut parser = Parser::default();
        parser.push(&[0x1e; 20_000]);
        assert!(parser.pending.len() <= 512);
    }

    #[test]
    fn stray_record_separators_preserve_output_and_resynchronize_on_the_next_frame() {
        let raw = b"before\x1e\x1e{\"ok\":true}\n\xff";
        for code in [0, 23] {
            let status = format!("n:STATUS:{code}");
            let tail = format!("\x1e{status}\x1f\x1en:END\x1f\x1es:RESTORED\x1f");
            let wire = [raw.as_slice(), tail.as_bytes()].concat();
            for split in 0..=wire.len() {
                let mut parser = Parser::default();
                let mut parts = parser.push(&wire[..split]);
                parts.extend(parser.push(&wire[split..]));
                let mut data = Vec::new();
                let mut frames = Vec::new();
                for part in parts {
                    match part {
                        Fragment::Data(bytes) => data.extend(bytes),
                        Fragment::Frame(frame) => frames.push(frame),
                    }
                }
                assert_eq!(data, raw);
                assert_eq!(frames, [status.as_str(), "n:END", "s:RESTORED"]);
            }
        }
    }
    #[test]
    fn full_input_ack_is_preserved_across_every_fragment_boundary() {
        let (line, _) = InputEncoding::Base64
            .confirmed_payload(&"x".repeat(INPUT_CHUNK_BYTES), "test")
            .next()
            .unwrap();
        let content = std::str::from_utf8(&line[1..line.len() - 1]).unwrap();
        let ack = format!("{}:INPUT:99999:240:{content}", "n".repeat(32));
        let wire = format!("\x1e{ack}\x1f");
        for split in 0..=wire.len() {
            let mut parser = Parser::default();
            let mut parts = parser.push(&wire.as_bytes()[..split]);
            parts.extend(parser.push(&wire.as_bytes()[split..]));
            assert_eq!(parts, vec![Fragment::Frame(ack.clone())]);
        }
    }
}
