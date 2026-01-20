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


SIMPLE_NAMESPACE_XML = """
<root xmlns="http://example.com/" xmlns:ns="http://ns.com/">
    <item>data</item>
    <ns:element>namespaced</ns:element>
</root>
"""


def test_namespace_ignore():
    compare_parsers(SIMPLE_NAMESPACE_XML, process_namespaces=False)


def test_basic_namespace():
    compare_parsers(SIMPLE_NAMESPACE_XML, process_namespaces=True)


def test_namespace_mapping():
    namespaces = {"http://example.com/": "ex", "http://ns.com/": "ns"}
    compare_parsers(SIMPLE_NAMESPACE_XML, process_namespaces=True, namespaces=namespaces)


def test_namespace_separator():
    namespaces = {"http://example.com/": "ex", "http://ns.com/": "ns"}
    compare_parsers(
        SIMPLE_NAMESPACE_XML,
        process_namespaces=True,
        namespaces=namespaces,
        namespace_separator="|",
    )


@pytest.mark.parametrize(
    "xml",
    [
        """
<root xmlns="http://example.com/">
    <level1>
        <level2 xmlns:ns="http://ns.com/">
            <ns:item>data</ns:item>
        </level2>
    </level1>
</root>
        """,
        """
<root xmlns="http://example.com/">
    <child xmlns="http://ns.com/">
        <item>data</item>
    </child>
</root>
        """,
        """
<root xmlns="http://example.com/"  xmlns:ns="http://ns.com/"  >
    <item>data</item>
    <ns:item>namespaced</ns:item>
</root>
        """,
        """
<root xmlns="http://example.com/">
    <empty />
    <ns:empty xmlns:ns="http://ns.com/" />
</root>
        """,
    ],
)
def test_resolve_namespaces(xml):
    namespaces = {"http://example.com/": "ex", "http://ns.com/": "ns"}
    compare_parsers(xml, process_namespaces=True, namespaces=namespaces)


@pytest.mark.parametrize("process_namespaces", [True, False])
def test_attrs_names(process_namespaces):
    xml = """
        <root xmlns:foo="http://example.com/foo" xmlns:bar="http://example.com/bar" bar:attr="1">
            <item></item>
            <foo:next_item></foo:next_item>
        </root>

    """
    compare_parsers(xml, process_namespaces=process_namespaces)


def test_custom_separator_deep():
    xml = """
<root xmlns="http://example.com/">
    <level1>
        <level2>
            <item>data</item>
        </level2>
    </level1>
</root>
    """
    namespaces = {"http://example.com/": "ex"}
    compare_parsers(xml, process_namespaces=True, namespaces=namespaces, namespace_separator="|")


def test_default_namespace_only():
    xml = """
<root xmlns="http://example.com/">
    <child>value</child>
</root>
    """
    compare_parsers(xml, process_namespaces=True)


def test_multiple_default_namespaces():
    xml = """
<root xmlns="http://example.com/">
    <child xmlns="http://other.com/">
        <item>value</item>
    </child>
</root>
    """
    compare_parsers(xml, process_namespaces=True)


def test_namespace_with_underscore_separator():
    xml = """
<root xmlns="http://example.com/">
    <item>data</item>
</root>
    """
    namespaces = {"http://example.com/": "ex"}
    compare_parsers(xml, process_namespaces=True, namespaces=namespaces, namespace_separator="_")


def test_namespace_on_attributes():
    xml = """
<root xmlns:ns="http://example.com/">
    <item ns:attr="value">content</item>
</root>
    """
    compare_parsers(xml, process_namespaces=True)


def test_empty_namespace_prefix():
    xml = """
<root xmlns="http://example.com/">
    <item>data</item>
</root>
    """
    namespaces = {"http://example.com/": ""}
    compare_parsers(xml, process_namespaces=True, namespaces=namespaces)
