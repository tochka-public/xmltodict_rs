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


XML_EXAMPLES = [
    "<root><a>1</a><b>2</b></root>",
    "<root><item>data</item></root>",
    "<root><a>1</a><!-- comment --><b>2</b></root>",
    "<root><a attr='x'>1</a><b>2</b></root>",
    "<root><c><d>3</d></c></root>",
    "<a>start<b>middle</b>end</a>",
]


@pytest.mark.parametrize("xml", XML_EXAMPLES)
def test_postprocessor_change_keys(xml):
    def post(path, key, value):
        return f"new_{key}", value

    compare_parsers(xml, postprocessor=post)


@pytest.mark.parametrize("xml", XML_EXAMPLES)
def test_postprocessor_change_values(xml):
    def post(path, key, value):
        if isinstance(value, str):
            return key, value.upper()
        return key, value

    compare_parsers(xml, postprocessor=post)


@pytest.mark.parametrize("xml", XML_EXAMPLES)
def test_postprocessor_skip_element(xml):
    def post(path, key, value):
        if key == "b":
            return None
        return key, value

    compare_parsers(xml, postprocessor=post)


@pytest.mark.parametrize("xml", XML_EXAMPLES)
def test_postprocessor_mixed_changes(xml):
    def post(path, key, value):
        if key == "a":
            return f"{key}_new", f"{value}_new"
        elif key == "b":
            return None
        return key, value

    compare_parsers(xml, postprocessor=post)


@pytest.mark.parametrize("xml", XML_EXAMPLES)
def test_postprocessor_with_comments_preserved(xml):
    def post(path, key, value):
        if key.startswith("#comment"):
            return key, value
        return f"{key}_x", value

    compare_parsers(xml, postprocessor=post, process_comments=True)


@pytest.mark.parametrize("xml", XML_EXAMPLES)
def test_postprocessor_various_value_types(xml):
    def post(path, key, value):
        if key == "a":
            return key, int(value)
        if key == "b":
            return key, [value]
        return key, value

    compare_parsers(xml, postprocessor=post)


def test_postprocessor_exception_propagation():
    xml = "<root><a>1</a><b>2</b></root>"

    def post(path, key, value):
        if key == "b":
            raise ValueError("Intentional error")
        return key, value

    with pytest.raises(ValueError, match="Intentional error"):
        xmltodict_rs.parse(xml, postprocessor=post)


def test_postprocessor_change_attribute_keys():
    xml = "<root><item id='1' name='test'>value</item></root>"

    def post(path, key, value):
        if isinstance(value, dict):
            new_value = {f"new_{k}": v for k, v in value.items()}
            return key, new_value
        return key, value

    compare_parsers(xml, postprocessor=post)


def test_postprocessor_change_attribute_values():
    xml = "<root><item id='1' name='test'>value</item></root>"

    def post(path, key, value):
        if isinstance(value, dict):
            new_value = {k: str(v).upper() for k, v in value.items()}
            return key, new_value
        return key, value

    compare_parsers(xml, postprocessor=post)


def test_postprocessor_skip_attributes():
    xml = "<root><item id='1' name='test'>value</item></root>"

    def post(path, key, value):
        if isinstance(value, dict):
            new_value = {k: v for k, v in value.items() if k != "id"}
            return key, new_value
        return key, value

    compare_parsers(xml, postprocessor=post)


def test_postprocessor_receives_path():
    paths_seen = []

    def post(path, key, value):
        paths_seen.append((list(path), key))
        return key, value

    xml = "<root><child><item>data</item></child></root>"
    xmltodict_rs.parse(xml, postprocessor=post)

    assert len(paths_seen) > 0
    assert any("root" in str(p) for p in paths_seen)


def test_postprocessor_with_force_cdata():
    def post(path, key, value):
        return key.upper(), value

    xml = "<root><item>data</item></root>"
    compare_parsers(xml, postprocessor=post, force_cdata=True)


def test_postprocessor_return_none_removes_element():
    def post(path, key, value):
        if key == "remove_me":
            return None
        return key, value

    xml = "<root><keep>1</keep><remove_me>2</remove_me></root>"
    result = xmltodict_rs.parse(xml, postprocessor=post)
    assert "remove_me" not in result["root"]
    assert result["root"]["keep"] == "1"


def test_postprocessor_convert_to_int():
    def post(path, key, value):
        if key == "number" and isinstance(value, str):
            return key, int(value)
        return key, value

    xml = "<root><number>42</number><text>hello</text></root>"
    result = xmltodict_rs.parse(xml, postprocessor=post)
    assert result["root"]["number"] == 42
    assert result["root"]["text"] == "hello"


def test_postprocessor_with_namespaces():
    def post(path, key, value):
        return key.replace(":", "_"), value

    xml = """
    <root xmlns:ns="http://example.com/">
        <ns:item>data</ns:item>
    </root>
    """
    compare_parsers(xml, postprocessor=post, process_namespaces=False)
