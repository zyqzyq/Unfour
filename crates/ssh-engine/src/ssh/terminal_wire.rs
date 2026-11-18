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
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            let Some(start) = self.pending.iter().position(|b| *b == 0x1e) else {
                if !self.pending.is_empty() {
                    out.push(Fragment::Data(std::mem::take(&mut self.pending)));
                }
                break;
            };
            if start > 0 {
                out.push(Fragment::Data(self.pending.drain(..start).collect()));
            }
            match self.pending.iter().position(|b| *b == 0x1f) {
                Some(end) if end <= 512 => {
                    let bytes: Vec<_> = self.pending.drain(..=end).collect();
                    match std::str::from_utf8(&bytes[1..end]) {
                        Ok(value) => out.push(Fragment::Frame(value.into())),
                        Err(_) => out.push(Fragment::Data(bytes)),
                    }
                }
                _ if self.pending.len() > 512 => {
                    out.push(Fragment::Data(self.pending.drain(..1).collect()))
                }
                _ => break,
            }
        }
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
