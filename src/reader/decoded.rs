#![allow(clippy::indexing_slicing, reason = "solid invariants with exhaustive checks")]
#![allow(clippy::single_match_else, reason = "breaks control flow otherwise")]
#![allow(clippy::unwrap_used, reason = "stream is always safe to unwrap")]

use crate::encoding::error::EncodingError;
use crate::encoding::{identify_encoding_from_opening_bytes, XML_DECL_END};
use crate::reader::pending::PendingBytes;
use encoding_rs::{Decoder, DecoderResult, Encoding, UTF_8};
use std::io::{self, Chain, Cursor, Read};

const INPUT_BUF_SIZE: usize = 8 * 1024;
const DECODE_BUF_SIZE: usize = 8 * 1024; // can't be too small. see Decoder infinite loops section.
const XML_DECL_SCAN_LIMIT: usize = 1024;

/// This reader treats `read(...) == 0` as final EOF and finalizes decoding.
/// Therefore, the underlying source must return empty bytes only at true end-of-stream.
/// Sources that return `0` for temporary starvation are intentionally unsupported.
pub struct DecodingReader<R: Read> {
    /// Input stream.
    ///
    /// If the caller provides an [`Encoding`], this chain is trivial: an empty cursor
    /// chained with the underlying stream.
    ///
    /// Otherwise, the encoding must be detected automatically. To do so, bytes are read
    /// from the stream into a temporary `prefetched` buffer and scanned until detection completes.
    /// The prefetched bytes are then merged back into `stream` by rebuilding the chain.
    ///
    /// `Option` is used so the current chain can be moved out, reconstructed, and stored back.
    /// This field is always `Some` when accessed.
    stream: Option<Chain<Cursor<Vec<u8>>, R>>,
    reached_eof: bool,
    /// Temporary buffer used for reads from the inner stream.
    buf: Vec<u8>,
    offset: usize,
    len: usize,
    /// Prefix of the input stream retained for encoding detection retries.
    /// Once the detection is done, it is merged back into the input stream.
    prefetched: Vec<u8>,
    /// Holds decoded bytes ready to be consumed.
    decoded: Vec<u8>,
    finished: bool,
    decoder: Option<Decoder>,
    pending: PendingBytes,
}

impl<R: Read> DecodingReader<R> {
    pub fn for_xml_input(inner: R, encoding: Option<&'static Encoding>) -> Self {
        Self {
            stream: Some(Cursor::new(Vec::new()).chain(inner)),
            reached_eof: false,
            buf: vec![0; INPUT_BUF_SIZE],
            offset: 0,
            len: 0,
            prefetched: Vec::new(),
            decoded: vec![0; DECODE_BUF_SIZE],
            decoder: encoding.map(Encoding::new_decoder),
            finished: false,
            pending: PendingBytes::default(),
        }
    }
}

impl<R: Read> Read for DecodingReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }

        if !self.pending.is_empty() {
            return Ok(self.pending.copy_into(out));
        }

        let chunk_len = self.decode_chunk()?;

        // downstream implementation guarantees that 0 <= n <= buf.len()
        let chunk = &self.decoded[..chunk_len];

        if chunk.len() <= out.len() {
            out[..chunk.len()].copy_from_slice(chunk);
            Ok(chunk.len())
        } else {
            let (head, tail) = chunk.split_at(out.len());
            out.copy_from_slice(head);
            self.pending.fill_from_slice(tail);
            Ok(head.len())
        }
    }
}

impl<R: Read> DecodingReader<R> {
    /// Detects the encoding from the current input prefix.
    ///
    /// Returns `Some(...)` once the encoding can be determined from the opening
    /// bytes or once it is safe to fall back to UTF-8.
    ///
    /// Returns `None` if the input appears to begin with an XML declaration that
    /// may still be incomplete and more bytes are needed.
    fn detect_encoding_from_prefix(buf: &[u8], reached_eof: bool) -> Option<&'static Encoding> {
        if let Some(enc) = identify_encoding_from_opening_bytes(buf) {
            return Some(enc);
        }

        let starts_with_xml_declaration = b"<?xml"
            .iter()
            .zip(buf)
            .all(|(a, b)| a.eq_ignore_ascii_case(b));

        let may_expect_declaration_ending = XML_DECL_END.find(buf).is_none()
            && buf.len() < XML_DECL_SCAN_LIMIT
            && !reached_eof;

        if starts_with_xml_declaration && may_expect_declaration_ending {
            None
        } else {
            Some(UTF_8) // give up
        }
    }

    /// Reads from the inner I/O stream until the encoding can be determined, or until
    /// `XML_DECL_SCAN_LIMIT` bytes have been prefetched.
    ///
    /// If the inner read is interrupted, [`DecodingReader::read`] can be safely retried
    /// without losing data.
    fn prefetch_and_init_decoder(&mut self) -> io::Result<()> {
        loop {
            match Self::detect_encoding_from_prefix(&self.prefetched, self.reached_eof) {
                Some(enc) => {
                    self.decoder = Some(enc.new_decoder());
                    self.stream = self.stream.take().map(|stream| {
                        let prefix = std::mem::take(&mut self.prefetched);
                        Cursor::new(prefix).chain(stream.into_inner().1)
                    });
                    return Ok(());
                }
                None => {
                    let read = self.stream.as_mut().unwrap().read(&mut self.buf)?;
                    match read {
                        // buffer is empty, which may indicate this is the final EOF
                        0 => self.reached_eof = true,
                        n => {
                            // downstream implementation guarantees that 0 <= n <= buf.len()
                            let new_bytes = &self.buf[..n];
                            self.prefetched.extend_from_slice(new_bytes);
                        }
                    }
                }
            }
        }
    }

    fn decode_chunk(&mut self) -> io::Result<usize> {
        if self.decoder.is_none() {
            self.prefetch_and_init_decoder()?;
        }

        loop {
            if self.offset >= self.len && !self.reached_eof {
                self.consume_stream()?;
            }

            let Some((result, read, produced)) = self.decode_input() else {
                return Ok(0);
            };
            self.offset = self.offset.saturating_add(read);

            match result {
                DecoderResult::Malformed(_, _) => {
                    let enc = self.decoder.as_ref().map_or(UTF_8, Decoder::encoding);
                    return Err(io::Error::other(EncodingError(enc)));
                }
                _ if produced > 0 => {
                    return Ok(produced);
                }
                DecoderResult::OutputFull => {
                    return Err(io::Error::other("Internal decoder buffer error"));
                }
                DecoderResult::InputEmpty => {
                    if !self.reached_eof {
                        self.consume_stream()?;
                    } else if self.offset >= self.len {
                        self.finished = true;
                    }
                }
            }
        }
    }

    fn consume_stream(&mut self) -> io::Result<()> {
        self.shift_unread_input();

        debug_assert!(!self.reached_eof);
        debug_assert!(
            self.len <= self.buf.len(),
            "inner reader must guarantee 0 <= n <= buf.len()"
        );

        let input = &mut self.buf[self.len..];
        let read = self.stream.as_mut().unwrap().read(input)?;
        if read == 0 {
            self.reached_eof = true;
        } else {
            self.len += read;
        }
        Ok(())
    }

    /// Shifts unread bytes of the input buffer closer to the start.
    ///
    /// The decoder may stop before consuming the entire input buffer, leaving a trailing
    /// incomplete sequence behind (for example, only part of a multibyte character).
    /// Those unread bytes are shifted to the start so that newly read bytes can be appended
    /// after them without wasting buffer space.
    fn shift_unread_input(&mut self) {
        if self.offset == 0 {
            return;
        }
        if self.offset < self.len {
            self.buf.copy_within(self.offset..self.len, 0);
            self.len -= self.offset;
        } else {
            self.len = 0;
        }
        self.offset = 0;
    }

    fn decode_input(&mut self) -> Option<(DecoderResult, usize, usize)> {
        let decoder = self.decoder.as_mut()?;
        if self.offset < self.len {
            let src = &self.buf[self.offset..self.len];
            Some(decoder.decode_to_utf8_without_replacement(
                src,
                &mut self.decoded,
                self.reached_eof,
            ))
        } else if self.reached_eof && !self.finished {
            // this final flush is critical to release any pending buffer in the decoder.
            Some(decoder.decode_to_utf8_without_replacement(&[], &mut self.decoded, true))
        } else {
            None
        }
    }
}
