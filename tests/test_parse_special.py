from xml.parsers.expat import ExpatError

import pytest
import xmltodict

import xmltodict_rs


def compare_parsers(xml: str, **kwargs) -> None:
    try:
        original = xmltodict.parse(xml, **kwargs)
    except Exception as e:
        with pytest.raises(type(e)):
            xmltodict_rs.parse(xml, **kwargs)
        return
    rust_impl = xmltodict_rs.parse(xml, **kwargs)
    assert rust_impl == original, (
        f"\nXML: {xml!r}\nKwargs: {kwargs}\nOriginal: {original!r}\nRust: {rust_impl!r}"
    )


# CDATA tests


def test_cdata_sections():
    xml = "<root><![CDATA[Some <b>HTML</b> content & entities]]></root>"
    result = xmltodict_rs.parse(xml)
    assert result["root"] == "Some <b>HTML</b> content & entities"


def test_cdata_with_mixed_content():
    xml = "<root>Text <![CDATA[<b>bold</b>]]> more text</root>"
    result = xmltodict_rs.parse(xml)
    assert "Text" in result["root"]
    assert "<b>bold</b>" in result["root"]
    assert "more text" in result["root"]


def test_multiple_cdata_sections():
    xml = "<root><![CDATA[First]]><![CDATA[Second]]></root>"
    result = xmltodict_rs.parse(xml)
    assert result["root"] == "FirstSecond"


# Comments tests


def test_comments_ignored_by_default():
    xml = """
    <a>
      <b>
        <!-- b comment -->
        <c>
            <!-- c comment -->
            1
        </c>
        <d>2</d>
      </b>
    </a>
    """
    expected = {
        "a": {
            "b": {
                "c": "1",
                "d": "2",
            },
        }
    }
    result = xmltodict_rs.parse(xml, process_comments=False)
    assert result == expected


def test_comments_processed_when_enabled():
    xml = """
    <a>
      <!-- root comment -->
      <b>
        <!-- b comment -->
        <c>1</c>
      </b>
    </a>
    """
    result = xmltodict_rs.parse(xml, process_comments=True)
    assert "a" in result
    assert "b" in result["a"]
    assert result["a"]["b"]["c"] == "1"


@pytest.mark.parametrize(
    "xml",
    [
        "<root><!-- single comment --><item>data</item></root>",
        "<root><!-- before --><item>data</item><!-- after --></root>",
        "<root><!-- c1 --><!-- c2 --><item>data</item><!-- c3 --></root>",
        "<root><a>1</a><!-- between --><b>2</b></root>",
        "<root><!-- root --><a><!-- a --><b>2</b></a></root>",
        "<root><!-- only comment --></root>",
        "<root>before<!-- comment -->after</root>",
        "<root><!-- c1 --><a>1</a><!-- c2 --><b>2</b><!-- c3 --></root>",
    ],
)
@pytest.mark.parametrize("strip_whitespace", [True, False])
def test_process_comments(xml, strip_whitespace):
    compare_parsers(xml, process_comments=True, strip_whitespace=strip_whitespace)


@pytest.mark.parametrize(
    "xml",
    [
        "<root><!-- single comment --><item>data</item></root>",
        "<root><!-- before --><item>data</item><!-- after --></root>",
    ],
)
def test_ignore_comments(xml):
    compare_parsers(xml, process_comments=False)


# Comments/PIs must break the current element's text run (regression: a
# comment or PI used to silently re-coalesce the text on either side of it,
# dropping the `cdata_separator` that should land between the two halves).


def test_comment_breaks_text_run():
    compare_parsers("<a>x<!--c-->y</a>", cdata_separator="|")


def test_comment_breaks_text_run_with_process_comments():
    compare_parsers("<a>x<!--c-->y</a>", cdata_separator="|", process_comments=True)


def test_pi_breaks_text_run():
    compare_parsers("<a>x<?pi?>y</a>", cdata_separator="|")


def test_comment_then_entity_does_not_recoalesce():
    compare_parsers("<a>x<!--c-->&amp;y</a>", cdata_separator="|")


# XML declaration tests


def test_xml_declaration_handling():
    xml = '<?xml version="1.0" encoding="UTF-8"?><root>content</root>'
    result = xmltodict_rs.parse(xml)
    assert result == {"root": "content"}


def test_processing_instructions():
    xml = '<?xml-stylesheet type="text/xsl" href="style.xsl"?><root>content</root>'
    result = xmltodict_rs.parse(xml)
    assert result == {"root": "content"}


def test_doctype_declaration():
    xml = """<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN">
    <root>content</root>"""
    result = xmltodict_rs.parse(xml)
    assert result == {"root": "content"}


# Long content tests


def test_very_long_attribute_values():
    long_value = "x" * 10000
    xml = f'<element attr="{long_value}">content</element>'
    result = xmltodict_rs.parse(xml)
    assert result["element"]["@attr"] == long_value


def test_very_long_text_content():
    long_content = "x" * 10000
    xml = f"<element>{long_content}</element>"
    result = xmltodict_rs.parse(xml)
    assert result["element"] == long_content


# Custom comment_key tests


def test_custom_comment_key():
    xml = "<root><!-- my comment --><item>data</item></root>"
    compare_parsers(xml, process_comments=True, comment_key="_COMMENT_")


def test_comment_key_with_multiple_comments():
    xml = "<root><!-- c1 --><a>1</a><!-- c2 --></root>"
    compare_parsers(xml, process_comments=True, comment_key="__comment__")


# Edge cases


def test_empty_comment():
    xml = "<root><!----><item>data</item></root>"
    compare_parsers(xml, process_comments=True)


def test_comment_with_dashes():
    xml = "<root><!-- comment - with - dashes --><item>data</item></root>"
    compare_parsers(xml, process_comments=True)


def test_cdata_empty():
    xml = "<root><![CDATA[]]></root>"
    result = xmltodict_rs.parse(xml)
    assert result["root"] is None or result["root"] == ""


def test_cdata_with_special_xml_chars():
    xml = "<root><![CDATA[<>&\"']]></root>"
    result = xmltodict_rs.parse(xml)
    assert result["root"] == "<>&\"'"


def test_nested_element_after_cdata():
    xml = "<root><![CDATA[text]]><child>data</child></root>"
    result = xmltodict_rs.parse(xml)
    assert "text" in str(result["root"])
    assert "child" in result["root"]


def test_xml_with_encoding_declaration():
    xml = '<?xml version="1.0" encoding="ISO-8859-1"?><root>data</root>'
    result = xmltodict_rs.parse(xml)
    assert result == {"root": "data"}


def test_multiple_processing_instructions():
    xml = """<?xml version="1.0"?>
    <?xml-stylesheet type="text/xsl" href="style.xsl"?>
    <?custom-pi data="value"?>
    <root>content</root>"""
    result = xmltodict_rs.parse(xml)
    assert result == {"root": "content"}


# Junk outside root tests


@pytest.mark.parametrize(
    "xml",
    [
        "<a>1</a>junk",
        "<a/><b/>",
        "<a/>text",
        "junk<a/>",
        "<a/><!DOCTYPE b>",
        '<a/><?xml version="1.0"?>',
    ],
)
def test_junk_outside_root_raises(xml):
    with pytest.raises(ExpatError):
        xmltodict.parse(xml)
    with pytest.raises(ExpatError):
        xmltodict_rs.parse(xml)


def test_pi_after_root_is_legal():
    # Unlike DOCTYPE/decl, a processing instruction after the root is legal
    # Misc content and must not be rejected as junk.
    compare_parsers("<a/><?pi?>")


@pytest.mark.parametrize("xml", ["<a/>\n", "  <a/>  ", "<a>1</a>\t\n"])
def test_whitespace_outside_root_is_legal(xml):
    assert xmltodict_rs.parse(xml) == xmltodict.parse(xml)
    # and with whitespace stripping disabled
    assert xmltodict_rs.parse(xml, strip_whitespace=False) == xmltodict.parse(
        xml, strip_whitespace=False
    )


def test_deeply_nested_parse():
    depth = 50_000
    xml = "<x>" * depth + "</x>" * depth
    result = xmltodict_rs.parse(xml)
    for _ in range(depth - 1):
        result = result["x"]
    assert result == {"x": None}


@pytest.mark.parametrize(
    "xml",
    [
        '<a><?xml version="1.0"?></a>',
        ' <?xml version="1.0"?><a/>',
        "<a><!DOCTYPE b></a>",
    ],
)
def test_misplaced_declaration_raises(xml):
    with pytest.raises(ExpatError):
        xmltodict.parse(xml)
    with pytest.raises(ExpatError):
        xmltodict_rs.parse(xml)


@pytest.mark.parametrize("xml", ['<?xml version="1.0"?><a/>', "<!DOCTYPE a><a/>"])
def test_prolog_declarations_are_legal(xml):
    assert xmltodict_rs.parse(xml) == xmltodict.parse(xml)
