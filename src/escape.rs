use std::borrow::Cow;
use std::slice::from_raw_parts;
use std::str::from_utf8_unchecked;

const LT: u8 = b'<';
const GT: u8 = b'>';
const AMPERSAND: u8 = b'&';

const ESCAPED_AMP: &str = "&amp;";
const ESCAPED_LT: &str = "&lt;";
const ESCAPED_GT: &str = "&gt;";

pub fn escape_xml(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let len = bytes.len();

    let need_escape = memchr::memchr3(AMPERSAND, LT, GT, bytes).is_some();

    if !need_escape {
        return Cow::Borrowed(text);
    }

    let mut i = 0;
    let mut last_pos = 0;
    let mut result = String::with_capacity(len * 6);

    let ptr = bytes.as_ptr();

    while i < len {
        let byte = unsafe {
            // SAFETY: `ptr` comes from `bytes.as_ptr()` which is valid for reads,
            // and `i` is bounded by `bytes.len()`, so `ptr.add(i)` is within bounds.
            *ptr.add(i)
        };
        match byte {
            AMPERSAND | LT | GT => {
                if last_pos < i {
                    let slice = unsafe {
                        // SAFETY: The slice from `last_pos` to `i` is valid UTF-8 because
                        // it's a subslice of the original `text` which is guaranteed to be valid UTF-8.
                        from_utf8_unchecked(from_raw_parts(ptr.add(last_pos), i - last_pos))
                    };
                    result.push_str(slice);
                }

                match byte {
                    AMPERSAND => result.push_str(ESCAPED_AMP),
                    LT => result.push_str(ESCAPED_LT),
                    GT => result.push_str(ESCAPED_GT),
                    _ => unreachable!(),
                }
                last_pos = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    if last_pos < len {
        let slice = unsafe {
            // SAFETY: The slice from `last_pos` to `bytes.len()` is valid UTF-8 because
            // it's a subslice of the original `text` which is guaranteed to be valid UTF-8.
            from_utf8_unchecked(from_raw_parts(ptr.add(last_pos), len - last_pos))
        };
        result.push_str(slice);
    }

    Cow::Owned(result)
}

pub fn escape_xml_attr(text: &str) -> Cow<'_, str> {
    let mut result: Option<String> = None;
    let mut last_pos = 0;

    for (i, ch) in text.char_indices() {
        match ch {
            '&' | '<' | '>' | '"' => {
                let is_first_escape = result.is_none();
                let s = result.get_or_insert_with(|| {
                    let mut output = String::with_capacity(text.len() + 20);
                    output.push_str(&text[..i]);
                    output
                });
                if !is_first_escape {
                    s.push_str(&text[last_pos..i]);
                }
                match ch {
                    '&' => s.push_str("&amp;"),
                    '<' => s.push_str("&lt;"),
                    '>' => s.push_str("&gt;"),
                    '"' => s.push_str("&quot;"),
                    _ => unreachable!(),
                }
                last_pos = i + ch.len_utf8();
            }
            _ => {}
        }
    }

    match result {
        None => Cow::Borrowed(text),
        Some(mut s) => {
            if last_pos < text.len() {
                s.push_str(&text[last_pos..]);
            }
            Cow::Owned(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(
            "Start &amp; then &lt; some &gt; text &amp; more &lt; text &gt; end",
            escape_xml("Start & then < some > text & more < text > end")
        );
    }

    #[test]
    fn test_escape_xml_no_escape_needed() {
        assert_eq!("Hello World", escape_xml("Hello World"));
    }

    #[test]
    fn test_escape_xml_attr() {
        assert_eq!(
            "value with &quot;quotes&quot; and &amp;",
            escape_xml_attr("value with \"quotes\" and &")
        );
    }
}
