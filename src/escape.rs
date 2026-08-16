use std::borrow::Cow;

const LT: u8 = b'<';
const GT: u8 = b'>';
const AMPERSAND: u8 = b'&';

const ESCAPED_AMP: &str = "&amp;";
const ESCAPED_LT: &str = "&lt;";
const ESCAPED_GT: &str = "&gt;";

pub fn escape_xml(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let mut positions = memchr::memchr3_iter(AMPERSAND, LT, GT, bytes).peekable();
    if positions.peek().is_none() {
        return Cow::Borrowed(text);
    }

    let mut result = String::with_capacity(text.len() + 24);
    let mut last = 0;
    for pos in positions {
        // memchr positions are on ASCII bytes, hence valid char boundaries
        if let Some(chunk) = text.get(last..pos) {
            result.push_str(chunk);
        }
        let escaped = match bytes.get(pos) {
            Some(&AMPERSAND) => ESCAPED_AMP,
            Some(&LT) => ESCAPED_LT,
            _ => ESCAPED_GT,
        };
        result.push_str(escaped);
        last = pos + 1;
    }
    if let Some(tail) = text.get(last..) {
        result.push_str(tail);
    }
    Cow::Owned(result)
}

pub fn escape_xml_attr(text: &str) -> Cow<'_, str> {
    let mut result: Option<String> = None;
    let mut last_pos = 0;

    for (i, ch) in text.char_indices() {
        match ch {
            '&' | '<' | '>' | '"' | '\n' | '\t' | '\r' => {
                let is_first_escape = result.is_none();
                let s = result.get_or_insert_with(|| {
                    let mut output = String::with_capacity(text.len() + 20);
                    output.push_str(&text[..i]);
                    output
                });
                if !is_first_escape {
                    s.push_str(&text[last_pos..i]);
                }
                let escaped = match ch {
                    '&' => "&amp;",
                    '<' => "&lt;",
                    '>' => "&gt;",
                    '"' => "&quot;",
                    '\n' => "&#10;",
                    '\t' => "&#9;",
                    _ => "&#13;",
                };
                s.push_str(escaped);
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

    #[test]
    fn test_escape_xml_attr_control_chars() {
        assert_eq!("a&#10;b&#9;c&#13;d", escape_xml_attr("a\nb\tc\rd"));
    }

    #[test]
    fn test_escape_xml_all_special() {
        assert_eq!("&amp;&lt;&gt;", escape_xml("&<>"));
    }

    #[test]
    fn test_escape_xml_multibyte_around_special() {
        assert_eq!("привет &amp; мир", escape_xml("привет & мир"));
    }

    #[test]
    fn test_escape_xml_empty() {
        assert_eq!("", escape_xml(""));
    }
}
