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
        "<a>data</a>",
        "<root>hello</root>",
        "<item>123</item>",
    ],
)
def test_force_cdata(xml):
    compare_parsers(xml, force_cdata=True)


def test_force_cdata_with_attributes():
    xml = '<element attr="value">text</element>'
    compare_parsers(xml, force_cdata=True)


@pytest.mark.parametrize(
    "xml",
    [
        "<a>data</a>",
        '<element attr="value">text</element>',
    ],
)
def test_custom_cdata_key(xml):
    compare_parsers(xml, force_cdata=True, cdata_key="_CONTENT_")


@pytest.mark.parametrize(
    "cdata_key",
    [
        "_TEXT_",
        "content",
        "value",
        "__text__",
    ],
)
def test_various_cdata_keys(cdata_key):
    xml = '<root attr="value">text<child>nested</child>more</root>'
    compare_parsers(xml, cdata_key=cdata_key)


@pytest.mark.parametrize(
    "xml",
    [
        '<a href="xyz"/>',
        '<root id="123"/>',
        '<item name="test" value="data"/>',
    ],
)
def test_custom_attr_prefix(xml):
    compare_parsers(xml, attr_prefix="!")


@pytest.mark.parametrize(
    "attr_prefix",
    [
        "!",
        "_",
        "$",
        "attr_",
        "",
    ],
)
def test_various_attr_prefixes(attr_prefix):
    xml = '<root attr="value">text</root>'
    compare_parsers(xml, attr_prefix=attr_prefix)


@pytest.mark.parametrize(
    "xml",
    [
        '<a href="xyz"/>',
        '<root id="123"/>',
        '<item name="test" value="data"/>',
        '<element attr1="val1" attr2="val2">content</element>',
    ],
)
def test_skip_attributes(xml):
    compare_parsers(xml, xml_attribs=False)


def test_xml_attribs_true():
    xml = '<root id="123">content</root>'
    compare_parsers(xml, xml_attribs=True)


def test_xml_attribs_false():
    xml = '<root id="123">content</root>'
    result = xmltodict_rs.parse(xml, xml_attribs=False)
    assert result == {"root": "content"}


@pytest.mark.parametrize(
    "xml",
    [
        "<root>  </root>",
        "<a>   text   </a>",
        "<element>\n  content  \n</element>",
    ],
)
@pytest.mark.parametrize("strip_whitespace", [True, False])
def test_whitespace_handling(xml, strip_whitespace):
    compare_parsers(xml, strip_whitespace=strip_whitespace)


def test_strip_whitespace_true():
    xml = "<root>  spaces  </root>"
    result = xmltodict_rs.parse(xml, strip_whitespace=True)
    assert result == {"root": "spaces"}


def test_strip_whitespace_false():
    xml = "<root>  spaces  </root>"
    result = xmltodict_rs.parse(xml, strip_whitespace=False)
    assert result == {"root": "  spaces  "}


@pytest.mark.parametrize(
    "separator",
    [
        "",
        " | ",
        "\n",
        " ",
        "---",
    ],
)
def test_custom_separators(separator):
    xml = "<a>start<b>middle</b>end</a>"
    compare_parsers(xml, cdata_separator=separator)


def test_cdata_separator_effect():
    xml = "<a>abc<b/>def</a>"
    result = xmltodict_rs.parse(xml, cdata_separator=" | ")
    assert result["a"]["#text"] == "abc | def"


@pytest.mark.parametrize(
    "param,value",
    [
        ("process_namespaces", True),
        ("process_namespaces", False),
        ("process_comments", True),
        ("process_comments", False),
        ("xml_attribs", True),
        ("xml_attribs", False),
        ("force_cdata", True),
        ("force_cdata", False),
        ("strip_whitespace", True),
        ("strip_whitespace", False),
    ],
)
def test_boolean_parameters(param, value):
    xml = '<root attr="value">text<child>nested</child>more</root>'
    kwargs = {param: value}
    try:
        original = xmltodict.parse(xml, **kwargs)
        rust_impl = xmltodict_rs.parse(xml, **kwargs)
        assert rust_impl == original, f"Mismatch with {param}={value}"
    except Exception as e:
        with pytest.raises(type(e)):
            xmltodict_rs.parse(xml, **kwargs)


@pytest.mark.parametrize(
    "kwargs",
    [
        {"attr_prefix": "!"},
        {"attr_prefix": "_"},
        {"cdata_key": "_TEXT_"},
        {"cdata_key": "content"},
        {"cdata_separator": " | "},
        {"cdata_separator": "\n"},
        {"comment_key": "_COMMENT_"},
    ],
)
def test_string_parameters(kwargs):
    xml = '<root attr="value">text<child>nested</child>more</root>'
    try:
        original = xmltodict.parse(xml, **kwargs)
        rust_impl = xmltodict_rs.parse(xml, **kwargs)
        assert rust_impl == original, f"Mismatch with {kwargs}"
    except Exception as e:
        with pytest.raises(type(e)):
            xmltodict_rs.parse(xml, **kwargs)


def test_force_cdata_with_custom_key():
    xml = '<root attr="value">text</root>'
    compare_parsers(xml, force_cdata=True, cdata_key="_VALUE_")


def test_custom_prefix_and_key():
    xml = '<root attr="value">text</root>'
    compare_parsers(xml, attr_prefix="!", cdata_key="_TEXT_")


def test_all_custom_parameters():
    xml = '<root attr="value">text<child>nested</child>more</root>'
    compare_parsers(
        xml,
        attr_prefix="$",
        cdata_key="_CONTENT_",
        cdata_separator=" ",
        strip_whitespace=True,
        force_cdata=True,
    )
