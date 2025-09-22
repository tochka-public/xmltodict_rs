"""
Compatibility tests comparing xmltodict_rs with original xmltodict using pytest
This ensures our Rust implementation produces identical results
"""

import pytest
import xmltodict
import xmltodict_rs


def compare_parsers(xml: str, **kwargs) -> None:
    """Compare xmltodict_rs.parse with xmltodict.parse"""
    try:
        original = xmltodict.parse(xml, **kwargs)
        rust_impl = xmltodict_rs.parse(xml, **kwargs)

        assert rust_impl == original, (
            f"\nXML: {xml!r}\nKwargs: {kwargs}\nOriginal: {original!r}\nRust:     {rust_impl!r}"
        )
    except Exception as e:
        # If original throws exception, rust should too
        with pytest.raises(type(e)):
            xmltodict_rs.parse(xml, **kwargs)


# Test data for parametrized tests
MINIMAL_ELEMENTS = [
    "<a/>",
    "<empty></empty>",
    "<root></root>",
]

SIMPLE_CONTENT = [
    "<a>data</a>",
    "<root>hello</root>",
    "<item>123</item>",
    "<text>  spaces  </text>",
]

ATTRIBUTE_CASES = [
    '<a href="xyz"/>',
    '<root id="123"/>',
    '<item name="test" value="data"/>',
    '<element attr1="val1" attr2="val2">content</element>',
]

REPEATED_ELEMENTS = [
    "<root><item>1</item><item>2</item><item>3</item></root>",
    "<catalog><book>Book1</book><book>Book2</book></catalog>",
]

MIXED_CONTENT = [
    "<a>start<b>middle</b>end</a>",
    "<root>before<item>data</item>after</root>",
]

NESTED_ELEMENTS = [
    "<root><child><grandchild>data</grandchild></child></root>",
    "<a><b><c><d>deep</d></c></b></a>",
]

WHITESPACE_CASES = [
    "<root>  </root>",
    "<a>   text   </a>",
    "<element>\n  content  \n</element>",
]

UNICODE_CASES = [
    "<root>Hello 世界</root>",
    "<element>Тест unicode содержимого</element>",
    f"<unicode>{chr(39321)}</unicode>",
]

SPECIAL_CHARS = [
    "<root>&lt;tag&gt; &amp; &quot;quotes&quot;</root>",
    "<element>&apos;apostrophe&apos;</element>",
]

ERROR_CASES = [
    "<root><unclosed>",  # Missing closing tag
    "<<invalid>>",  # Invalid syntax
    "",  # Empty string
]


@pytest.mark.parametrize("xml", MINIMAL_ELEMENTS)
def test_minimal_elements(xml):
    """Test minimal XML elements"""
    compare_parsers(xml)


@pytest.mark.parametrize("xml", SIMPLE_CONTENT)
def test_simple_content(xml):
    """Test elements with simple text content"""
    compare_parsers(xml)


@pytest.mark.parametrize("xml", ATTRIBUTE_CASES)
def test_attributes(xml):
    """Test attribute handling"""
    compare_parsers(xml)


@pytest.mark.parametrize("xml", SIMPLE_CONTENT)
def test_force_cdata(xml):
    """Test force_cdata parameter"""
    compare_parsers(xml, force_cdata=True)


@pytest.mark.parametrize("xml", ["<a>data</a>", '<element attr="value">text</element>'])
def test_custom_cdata_key(xml):
    """Test custom CDATA key"""
    compare_parsers(xml, force_cdata=True, cdata_key="_CONTENT_")


@pytest.mark.parametrize("xml", ATTRIBUTE_CASES)
def test_custom_attr_prefix(xml):
    """Test custom attribute prefix"""
    compare_parsers(xml, attr_prefix="!")


@pytest.mark.parametrize("xml", ATTRIBUTE_CASES)
def test_skip_attributes(xml):
    """Test skipping attributes"""
    compare_parsers(xml, xml_attribs=False)


@pytest.mark.parametrize("xml", REPEATED_ELEMENTS)
def test_repeated_elements(xml):
    """Test automatic list creation"""
    compare_parsers(xml)


@pytest.mark.parametrize("xml", MIXED_CONTENT)
def test_mixed_content(xml):
    """Test mixed content (text + elements)"""
    compare_parsers(xml)


@pytest.mark.parametrize("xml", NESTED_ELEMENTS)
def test_nested_elements(xml):
    """Test nested element structures"""
    compare_parsers(xml)


@pytest.mark.parametrize("xml", WHITESPACE_CASES)
@pytest.mark.parametrize("strip_whitespace", [True, False])
def test_whitespace_handling(xml, strip_whitespace):
    """Test whitespace stripping"""
    compare_parsers(xml, strip_whitespace=strip_whitespace)


@pytest.mark.parametrize("xml", ["<a>start<b>middle</b>end</a>"])
@pytest.mark.parametrize("separator", ["", " | ", "\n"])
def test_custom_separators(xml, separator):
    """Test custom separators for mixed content"""
    compare_parsers(xml, cdata_separator=separator)


@pytest.mark.parametrize("xml", UNICODE_CASES)
def test_unicode_content(xml):
    """Test Unicode character handling"""
    compare_parsers(xml)


@pytest.mark.parametrize("xml", SPECIAL_CHARS)
def test_special_characters(xml):
    """Test XML special characters and entities"""
    compare_parsers(xml)


@pytest.mark.parametrize(
    "xml",
    [
        "<root><!-- single comment --><item>data</item></root>",  # in root
        "<root><!-- before --><item>data</item><!-- after --></root>",  # before and after tag
        "<root><!-- c1 --><!-- c2 --><item>data</item><!-- c3 --></root>",  # multiple comments in a row
        "<root><a>1</a><!-- between --><b>2</b></root>",  # comments between tags
        "<root><!-- root --><a><!-- a --><b>2</b></a></root>",  # comments at different levels of nesting
        "<root><!-- only comment --></root>",  # only comment
        "<root>before<!-- comment -->after</root>",  # comments with text before/after
        "<root><!-- c1 --><a>1</a><!-- c2 --><b>2</b><!-- c3 --></root>",  # multiple comments and tags
    ],
)
@pytest.mark.parametrize("strip_whitespace", [True, False])
def test_process_comments(xml, strip_whitespace):
    """Test comment handling with process_comments=True"""
    compare_parsers(xml, process_comments=True, strip_whitespace=strip_whitespace)


@pytest.mark.parametrize(
    "xml",
    [
        "<root><!-- single comment --><item>data</item></root>",  # in root
        "<root><!-- before --><item>data</item><!-- after --></root>",  # before and after tag
    ],
)
def test_ignore_comments(xml):
    """Test comment handling with process_comments=False"""
    compare_parsers(xml, process_comments=False)


def test_complex_real_world():
    """Test complex real-world XML structure"""
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


def test_bytes_vs_string_input():
    """Test parsing bytes vs string input"""
    xml_str = "<root><item>test</item></root>"
    xml_bytes = xml_str.encode("utf-8")

    # Both should work and produce same result
    str_result = xmltodict.parse(xml_str)
    bytes_result = xmltodict.parse(xml_bytes)
    assert str_result == bytes_result

    # Our implementation should match
    compare_parsers(xml_str)

    # Test with bytes directly
    rust_bytes_result = xmltodict_rs.parse(xml_bytes)
    assert rust_bytes_result == str_result


@pytest.mark.parametrize("xml", ERROR_CASES)
def test_error_cases(xml):
    """Test error handling for malformed XML"""
    # Both should raise exceptions for malformed XML
    with pytest.raises(Exception):  # noqa: B017
        xmltodict.parse(xml)
    with pytest.raises(Exception):  # noqa: B017
        xmltodict_rs.parse(xml)


class TestParameterCompatibility:
    """Test all parameter combinations for compatibility"""

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
    def test_boolean_parameters(self, param, value):
        """Test each boolean parameter individually"""
        xml = '<root attr="value">text<child>nested</child>more</root>'
        kwargs = {param: value}

        try:
            original = xmltodict.parse(xml, **kwargs)
            rust_impl = xmltodict_rs.parse(xml, **kwargs)
            assert rust_impl == original, f"Mismatch with {param}={value}"
        except Exception as e:
            # If original fails, rust should fail too
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
    def test_string_parameters(self, kwargs):
        """Test string parameter variations"""
        xml = '<root attr="value">text<child>nested</child>more</root>'

        try:
            original = xmltodict.parse(xml, **kwargs)
            rust_impl = xmltodict_rs.parse(xml, **kwargs)
            assert rust_impl == original, f"Mismatch with {kwargs}"
        except Exception as e:
            with pytest.raises(type(e)):
                xmltodict_rs.parse(xml, **kwargs)


# Focus tests for current issues
class TestCurrentIssues:
    """Focused tests for currently failing cases"""

    def test_empty_self_closing_tag(self):
        """Test empty self-closing tag handling"""
        xml = "<root><item/></root>"
        result = xmltodict_rs.parse(xml)
        assert result == {"root": {"item": None}}

    def test_self_closing_with_attributes(self):
        """Test self-closing tag with attributes"""
        xml = '<root><item id="123"/></root>'
        result = xmltodict_rs.parse(xml)
        assert result == {"root": {"item": {"@id": "123"}}}

    def test_multiple_attributes(self):
        """Test multiple attributes on element"""
        xml = '<root><item id="123" name="test"/></root>'
        result = xmltodict_rs.parse(xml)
        assert result == {"root": {"item": {"@id": "123", "@name": "test"}}}


class TestForceList:
    """Test force_list functionality"""

    @pytest.mark.parametrize(
        "force_list_value,expected_type",
        [
            (True, list),
            (False, str),
            (("server",), list),
            (["server"], list),
            ({"server"}, list),
            ({"server"}, list),
            (None, str),
        ],
    )
    def test_force_list_various_types(self, force_list_value, expected_type):
        """Test force_list with various types of values"""
        xml = "<servers><server>test</server></servers>"
        result = xmltodict_rs.parse(xml, force_list=force_list_value)

        assert "servers" in result
        assert "server" in result["servers"]
        assert isinstance(result["servers"]["server"], expected_type)

        if expected_type is list:
            assert result["servers"]["server"] == ["test"]
        else:
            assert result["servers"]["server"] == "test"

    @pytest.mark.parametrize(
        "xml,force_list,expected_structure",
        [
            # Single element should become list
            (
                "<servers><server>test</server></servers>",
                ("server",),
                {"servers": {"server": ["test"]}},
            ),
            # Multiple elements should be list
            (
                "<servers><server>1</server><server>2</server></servers>",
                ("server",),
                {"servers": {"server": ["1", "2"]}},
            ),
            # Mixed content - some forced, some not
            (
                """<config>
            <servers><server>test</server></servers>
            <settings><setting>value</setting></settings>
        </config>""",
                ("server",),
                {"config": {"servers": {"server": ["test"]}, "settings": {"setting": "value"}}},
            ),
            # Nested structure
            (
                """<root>
            <level1>
                <level2>
                    <item>data</item>
                </level2>
            </level1>
        </root>""",
                ("item",),
                {"root": {"level1": {"level2": {"item": ["data"]}}}},
            ),
        ],
    )
    def test_force_list_structures(self, xml, force_list, expected_structure):
        """Test force_list with various XML structures"""
        result = xmltodict_rs.parse(xml, force_list=force_list)
        assert result == expected_structure

    def test_force_list_callable_function(self):
        """Test force_list with callable function"""

        def force_list_func(path, key, value):
            return key == "server"

        xml = "<servers><server>test</server><item>other</item></servers>"
        result = xmltodict_rs.parse(xml, force_list=force_list_func)

        assert "servers" in result
        assert isinstance(result["servers"]["server"], list)
        assert result["servers"]["server"] == ["test"]
        assert isinstance(result["servers"]["item"], str)
        assert result["servers"]["item"] == "other"

    def test_force_list_callable_with_path(self):
        """Test force_list with callable that receives path and key"""

        def force_list_func(path, key, value):
            # Only force list for 'server' under 'servers' path
            if key != "server":
                return False
            return path and len(path) > 0 and path[-1] == "servers"

        xml = """<config>
            <servers><server>test</server></servers>
            <other><server>ignored</server></other>
        </config>"""
        result = xmltodict_rs.parse(xml, force_list=force_list_func)

        assert "config" in result
        assert isinstance(result["config"]["servers"]["server"], list)
        assert result["config"]["servers"]["server"] == ["test"]
        assert isinstance(result["config"]["other"]["server"], str)
        assert result["config"]["other"]["server"] == "ignored"

    def test_force_list_callable_errors(self):
        """Test that errors from force_list callable propagate to Python"""

        def force_list_func(path, key, value):
            raise ValueError("Test error from force_list function")

        xml = "<servers><server>test</server></servers>"

        with pytest.raises(ValueError, match="Test error from force_list function"):
            xmltodict_rs.parse(xml, force_list=force_list_func)

    def test_force_list_invalid_types(self):
        """Test force_list with invalid types that should raise errors"""
        xml = "<servers><server>test</server></servers>"

        # Test with object that doesn't support __contains__ or __call__
        class InvalidForceList:
            pass

        with pytest.raises(Exception):  # noqa: B017
            xmltodict_rs.parse(xml, force_list=InvalidForceList())

    def test_force_list_compatibility_with_original(self):
        """Test that force_list produces identical results to original xmltodict"""
        test_cases = [
            ("<root><item>1</item><item>2</item></root>", ("item",)),
            ("<config><servers><server>test</server></servers></config>", ("server",)),
        ]

        for xml, force_list in test_cases:
            original = xmltodict.parse(xml, force_list=force_list)
            rust = xmltodict_rs.parse(xml, force_list=force_list)
            assert rust == original, f"Mismatch for XML: {xml}, force_list: {force_list}"


class TestNamespaces:
    """Test namespaces functionality"""

    @pytest.fixture()
    def simple_namespace_xml(self) -> str:
        return """
    <root xmlns="http://example.com/" xmlns:ns="http://ns.com/">
        <item>data</item>
        <ns:element>namespaced</ns:element>
    </root>
    """

    def test_namespace_ignore(self, simple_namespace_xml):
        """Test namespace handling when process_namespaces=False"""
        compare_parsers(simple_namespace_xml, process_namespaces=False)

    def test_basic_namespace(self, simple_namespace_xml):
        """Test basic namespace functionality"""

        compare_parsers(simple_namespace_xml, process_namespaces=True)

    def test_namespace_mapping(self, simple_namespace_xml):
        """Test namespace mapping functionality"""

        namespaces = {"http://example.com/": "ex", "http://ns.com/": "ns"}

        compare_parsers(simple_namespace_xml, process_namespaces=True, namespaces=namespaces)

    def test_namespace_separator(self, simple_namespace_xml):
        """Test custom namespace separator"""

        namespaces = {"http://example.com/": "ex", "http://ns.com/": "ns"}

        compare_parsers(
            simple_namespace_xml,
            process_namespaces=True,
            namespaces=namespaces,
            namespace_separator="|",
        )

    @pytest.mark.parametrize(
        "xml",
        [
            # nested_namespace
            """
<root xmlns="http://example.com/"> 
    <level1>
        <level2 xmlns:ns="http://ns.com/">
            <ns:item>data</ns:item>
        </level2>
    </level1>
</root>
        """,
            # override_namespace
            """
<root xmlns="http://example.com/">
    <child xmlns="http://ns.com/">
        <item>data</item>
    </child>
</root>
        """,
            # mixed_prefix_elements
            """
<root xmlns="http://example.com/"  xmlns:ns="http://ns.com/"  >
    <item>data</item>
    <ns:item>namespaced</ns:item>
</root>
        """,
            # empty_elements_namespace
            """
<root xmlns="http://example.com/">
    <empty />
    <ns:empty xmlns:ns="http://ns.com/" />
</root>
        """,
        ],
    )
    def test_resolve_namespaces(self, xml):
        namespaces = {"http://example.com/": "ex", "http://ns.com/": "ns"}
        compare_parsers(xml, process_namespaces=True, namespaces=namespaces)

    @pytest.mark.parametrize("process_namespaces", [True, False])
    def test_attrs_names(self, process_namespaces):
        xml = """
            <root xmlns:foo="http://example.com/foo" xmlns:bar="http://example.com/bar" bar:attr="1">
                <item></item>
                <foo:next_item></foo:next_item>
            </root>
            
        """
        compare_parsers(xml, process_namespaces=process_namespaces)

    def test_custom_separator_deep(self):
        """Test custom namespace separator in nested elements"""
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
        compare_parsers(
            xml, process_namespaces=True, namespaces=namespaces, namespace_separator="|"
        )


class TestPostprocessors:
    @pytest.fixture(
        params=[
            "<root><a>1</a><b>2</b></root>",
            "<root><item>data</item></root>",
            "<root><a>1</a><!-- comment --><b>2</b></root>",
            "<root><a attr='x'>1</a><b>2</b></root>",
            "<root><c><d>3</d></c></root>",
            "<a>start<b>middle</b>end</a>",
        ]
    )
    def xml_example(self, request):
        """Provide various XML examples for postprocessor tests"""
        return request.param

    def test_change_keys(self, xml_example):
        """Postprocessor changes key names"""

        def post(path, key, value):
            return f"new_{key}", value

        compare_parsers(xml_example, postprocessor=post)

    def test_change_values(self, xml_example):
        """Postprocessor changes values"""

        def post(path, key, value):
            if isinstance(value, str):
                return key, value.upper()
            return key, value

        compare_parsers(xml_example, postprocessor=post)

    def test_skip_element(self, xml_example):
        """Postprocessor can skip elements"""

        def post(path, key, value):
            if key == "b":
                return None
            return key, value

        compare_parsers(xml_example, postprocessor=post)

    def test_mixed_changes(self, xml_example):
        """Postprocessor can change keys and values and skip others"""

        def post(path, key, value):
            if key == "a":
                return f"{key}_new", f"{value}_new"
            elif key == "b":
                return None
            return key, value

        compare_parsers(xml_example, postprocessor=post)

    def test_with_comments_preserved(self, xml_example):
        """Postprocessor affects elements but leaves comments unchanged"""

        def post(path, key, value):
            if key.startswith("#comment"):
                return key, value
            return f"{key}_x", value

        compare_parsers(xml_example, postprocessor=post, process_comments=True)

    def test_various_value_types(self, xml_example):
        """Postprocessor returns different Python types"""

        def post(path, key, value):
            if key == "a":
                return key, int(value)
            if key == "b":
                return key, [value]
            return key, value

        compare_parsers(xml_example, postprocessor=post)

    def test_exception_propagation(self):
        """Postprocessor exceptions propagate"""
        xml = "<root><a>1</a><b>2</b></root>"

        def post(path, key, value):
            if key == "b":
                raise ValueError("Intentional error")
            return key, value

        with pytest.raises(ValueError, match="Intentional error"):
            xmltodict_rs.parse(xml, postprocessor=post)

    @pytest.fixture
    def xml_with_attributes(self):
        return "<root><item id='1' name='test'>value</item></root>"

    def test_postprocessor_change_attribute_keys(self, xml_with_attributes):
        """Postprocessor changes attribute keys"""

        def post(path, key, value):
            if isinstance(value, dict):
                new_value = {f"new_{k}": v for k, v in value.items()}
                return key, new_value
            return key, value

        compare_parsers(xml_with_attributes, postprocessor=post)

    def test_postprocessor_change_attribute_values(self, xml_with_attributes):
        """Postprocessor changes attribute values"""

        def post(path, key, value):
            if isinstance(value, dict):
                new_value = {k: str(v).upper() for k, v in value.items()}
                return key, new_value
            return key, value

        compare_parsers(xml_with_attributes, postprocessor=post)

    def test_postprocessor_skip_attributes(self, xml_with_attributes):
        """Postprocessor can remove certain attributes"""

        def post(path, key, value):
            if isinstance(value, dict):
                new_value = {k: v for k, v in value.items() if k != "id"}
                return key, new_value
            return key, value

        compare_parsers(xml_with_attributes, postprocessor=post)


if __name__ == "__main__":
    # Run with pytest
    pytest.main([__file__, "-v"])
