#![allow(clippy::expect_used, clippy::indexing_slicing)]

use std::io::{self, Read};

#[allow(dead_code)]
mod encoding {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/encoding.rs"));
}

mod reader {
    pub mod pending {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/reader/pending.rs"
        ));
    }
}

#[path = "../src/reader/decoded.rs"]
mod decoded;

use decoded::DecodingReader;
use encoding_rs::WINDOWS_1251;

struct InterruptedOnceRead {
    data: Vec<u8>,
    pos: usize,
    call_count: usize,
    interrupt_on_call: usize,
    max_chunk: usize,
}

impl InterruptedOnceRead {
    fn new(data: Vec<u8>, interrupt_on_call: usize, max_chunk: usize) -> Self {
        Self {
            data,
            pos: 0,
            call_count: 0,
            interrupt_on_call,
            max_chunk,
        }
    }
}

impl Read for InterruptedOnceRead {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.call_count = self.call_count.saturating_add(1);
        if self.call_count == self.interrupt_on_call {
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }

        if self.pos >= self.data.len() {
            return Ok(0);
        }

        let remaining = self.data.len().saturating_sub(self.pos);
        let to_copy = out.len().min(remaining).min(self.max_chunk);
        let Some(src) = self.data.get(self.pos..self.pos + to_copy) else {
            return Err(io::Error::other("Internal source buffer error"));
        };
        let Some(dst) = out.get_mut(..to_copy) else {
            return Err(io::Error::other("Internal destination buffer error"));
        };

        dst.copy_from_slice(src);
        self.pos = self.pos.saturating_add(to_copy);
        Ok(to_copy)
    }
}

#[test]
fn interrupted_during_prefetch_can_be_retried_without_data_loss() {
    let original =
        r#"<?xml version="1.0" encoding="windows-1251"?><root><item>Привет</item></root>"#;
    let (encoded, _, had_errors) = WINDOWS_1251.encode(original);
    assert!(!had_errors);

    // Interrupt on the second read call while auto-detection is still prefetching.
    let source = InterruptedOnceRead::new(encoded.into_owned(), 2, 7);
    let mut reader = DecodingReader::for_xml_input(source, None);

    let mut out = Vec::new();
    let mut buf = [0_u8; 16];

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => (),
            Err(err) => panic!("unexpected read error: {err}"),
        }
    }

    let decoded = String::from_utf8(out).expect("decoded output must be valid UTF-8");
    assert_eq!(decoded, original);
}
