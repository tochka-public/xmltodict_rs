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


@pytest.mark.parametrize(
    "xml",
    [
        "<a/>",
        "<empty></empty>",
        "<root></root>",
        "<self-closing/>",
    ],
)
def test_empty_elements(xml):
    compare_parsers(xml)


def test_empty_self_closing_tag():
    xml = "<root><item/></root>"
    result = xmltodict_rs.parse(xml)
    assert result == {"root": {"item": None}}


def test_empty_elements_variations():
    assert xmltodict_rs.parse("<empty/>") == {"empty": None}
    assert xmltodict_rs.parse("<empty></empty>") == {"empty": None}
    assert xmltodict_rs.parse('<empty attr="value"/>') == {"empty": {"@attr": "value"}}
    assert xmltodict_rs.parse("<empty>   </empty>") == {"empty": None}


def test_empty_elements_in_lists():
    xml = """
    <root>
        <item>1</item>
        <item/>
        <item>3</item>
        <item></item>
    </root>
    """
    result = xmltodict_rs.parse(xml)
    items = result["root"]["item"]
    assert isinstance(items, list)
    assert len(items) == 4
    assert items[0] == "1"
    assert items[1] is None
    assert items[2] == "3"
    assert items[3] is None


@pytest.mark.parametrize(
    "xml",
    [
        "<a>data</a>",
        "<root>hello</root>",
        "<item>123</item>",
        "<text>  spaces  </text>",
    ],
)
def test_simple_content(xml):
    compare_parsers(xml)


@pytest.mark.parametrize(
    "xml",
    [
        '<a href="xyz"/>',
        '<root id="123"/>',
        '<item name="test" value="data"/>',
        '<element attr1="val1" attr2="val2">content</element>',
    ],
)
def test_attributes(xml):
    compare_parsers(xml)


def test_self_closing_with_attributes():
    xml = '<root><item id="123"/></root>'
    result = xmltodict_rs.parse(xml)
    assert result == {"root": {"item": {"@id": "123"}}}


def test_multiple_attributes():
    xml = '<root><item id="123" name="test"/></root>'
    result = xmltodict_rs.parse(xml)
    assert result == {"root": {"item": {"@id": "123", "@name": "test"}}}


def test_attribute_value_types():
    xml = """
    <config
        name="test"
        number="123"
        boolean="true"
        decimal="45.67"
        empty=""
        spaces="  spaced  ">
        content
    </config>
    """
    result = xmltodict_rs.parse(xml)
    config = result["config"]
    assert config["@name"] == "test"
    assert config["@number"] == "123"
    assert config["@boolean"] == "true"
    assert config["@decimal"] == "45.67"
    assert config["@empty"] == ""
    assert config["@spaces"] == "  spaced  "
    assert config["#text"] == "content"


def test_attributes_with_special_chars():
    xml = """
    <element
        normal="value"
        with_quotes="&quot;quoted&quot;"
        with_amp="&amp;"
        with_lt="&lt;"
        with_gt="&gt;">
        content
    </element>
    """
    result = xmltodict_rs.parse(xml)
    element = result["element"]
    assert element["@normal"] == "value"
    assert "@with_quotes" in element
    assert "@with_amp" in element
    assert "@with_lt" in element
    assert "@with_gt" in element


def test_attributes_with_unicode():
    xml = '<element attr="Hello 世界" attr2="Тест">content</element>'
    result = xmltodict_rs.parse(xml)
    element = result["element"]
    assert element["@attr"] == "Hello 世界"
    assert element["@attr2"] == "Тест"
    assert element["#text"] == "content"


@pytest.mark.parametrize(
    "xml",
    [
        "<root><item>1</item><item>2</item><item>3</item></root>",
        "<catalog><book>Book1</book><book>Book2</book></catalog>",
    ],
)
def test_repeated_elements(xml):
    compare_parsers(xml)


@pytest.mark.parametrize(
    "xml",
    [
        "<root><child><grandchild>data</grandchild></child></root>",
        "<a><b><c><d>deep</d></c></b></a>",
    ],
)
def test_nested_elements(xml):
    compare_parsers(xml)


def test_deeply_nested_structure():
    xml = """
    <level1>
        <level2>
            <level3>
                <level4>
                    <level5>
                        <level6>deep content</level6>
                    </level5>
                </level4>
            </level3>
        </level2>
    </level1>
    """
    result = xmltodict_rs.parse(xml)
    current = result
    for level in ["level1", "level2", "level3", "level4", "level5"]:
        assert level in current
        current = current[level]
    assert current["level6"] == "deep content"


def test_deep_nesting_with_attributes():
    xml = """
    <level1 id="1">
        <level2 id="2">
            <level3 id="3">
                <level4 id="4">
                    <level5 id="5">
                        <level6 id="6">deep content</level6>
                    </level5>
                </level4>
            </level3>
        </level2>
    </level1>
    """
    result = xmltodict_rs.parse(xml)
    current = result
    for i, level in enumerate(["level1", "level2", "level3", "level4", "level5"], 1):
        assert level in current
        assert current[level]["@id"] == str(i)
        current = current[level]
    assert current["level6"]["@id"] == "6"
    assert current["level6"]["#text"] == "deep content"


@pytest.mark.parametrize(
    "xml",
    [
        "<a>start<b>middle</b>end</a>",
        "<root>before<item>data</item>after</root>",
    ],
)
def test_mixed_content(xml):
    compare_parsers(xml)


def test_semi_structured_basic():
    xml = "<a>abc<b/>def</a>"
    result = xmltodict_rs.parse(xml)
    assert result["a"]["#text"] == "abcdef"
    assert result["a"]["b"] is None


def test_nested_semi_structured():
    xml = "<a>abc<b>123<c/>456</b>def</a>"
    result = xmltodict_rs.parse(xml)
    assert result["a"]["#text"] == "abcdef"
    assert result["a"]["b"]["#text"] == "123456"
    assert result["a"]["b"]["c"] is None


def test_complex_mixed_content():
    xml = """
    <article>
        This is the introduction.
        <paragraph>First paragraph with <emphasis>emphasized text</emphasis> and more content.</paragraph>
        Some text between paragraphs.
        <paragraph>Second paragraph.</paragraph>
        Final conclusion.
    </article>
    """
    result = xmltodict_rs.parse(xml)
    article = result["article"]
    assert "#text" in article
    paragraphs = article["paragraph"]
    assert isinstance(paragraphs, list)
    assert len(paragraphs) == 2
    first_para = paragraphs[0]
    assert "#text" in first_para
    assert "emphasis" in first_para


@pytest.mark.parametrize(
    "xml",
    [
        "<root>Hello 世界</root>",
        "<element>Тест unicode содержимого</element>",
        f"<unicode>{chr(39321)}</unicode>",
    ],
)
def test_unicode_content(xml):
    compare_parsers(xml)


@pytest.mark.parametrize(
    "xml",
    [
        "<root>&lt;tag&gt; &amp; &quot;quotes&quot;</root>",
        "<element>&apos;apostrophe&apos;</element>",
    ],
)
def test_special_characters(xml):
    compare_parsers(xml)


def test_complex_real_world():
    xml = """
    <catalog version="2.0">
        <products>
            <product id="p1" category="books">
                <name>Python Programming</name>
                <author>John Doe</author>
                <price currency="USD">29.99</price>
                <tags>
                    <tag>programming</tag>
                    <tag>python</tag>
                </tags>
            </product>
            <product id="p2" category="books">
                <name>Rust Systems Programming</name>
                <author>Jane Smith</author>
                <price currency="EUR">34.50</price>
            </product>
        </products>
    </catalog>
    """
    compare_parsers(xml)


@pytest.mark.parametrize(
    "xml",
    [
        "<root><unclosed>",
        "<<invalid>>",
        "",
    ],
)
def test_error_cases(xml):
    with pytest.raises(Exception):  # noqa: B017
        xmltodict.parse(xml)
    with pytest.raises(Exception):  # noqa: B017
        xmltodict_rs.parse(xml)


def test_bytes_vs_string_input():
    xml_str = "<root><item>test</item></root>"
    xml_bytes = xml_str.encode("utf-8")

    str_result = xmltodict.parse(xml_str)
    bytes_result = xmltodict.parse(xml_bytes)
    assert str_result == bytes_result

    compare_parsers(xml_str)

    rust_bytes_result = xmltodict_rs.parse(xml_bytes)
    assert rust_bytes_result == str_result
