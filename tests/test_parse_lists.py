import pytest
import xmltodict

import xmltodict_rs


@pytest.mark.parametrize(
    "force_list_value,expected_type",
    [
        (True, list),
        (False, str),
        (("server",), list),
        (["server"], list),
        ({"server"}, list),
        (None, str),
    ],
)
def test_force_list_various_types(force_list_value, expected_type):
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
        (
            "<servers><server>test</server></servers>",
            ("server",),
            {"servers": {"server": ["test"]}},
        ),
        (
            "<servers><server>1</server><server>2</server></servers>",
            ("server",),
            {"servers": {"server": ["1", "2"]}},
        ),
        (
            """<config>
        <servers><server>test</server></servers>
        <settings><setting>value</setting></settings>
    </config>""",
            ("server",),
            {"config": {"servers": {"server": ["test"]}, "settings": {"setting": "value"}}},
        ),
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
def test_force_list_structures(xml, force_list, expected_structure):
    result = xmltodict_rs.parse(xml, force_list=force_list)
    assert result == expected_structure


def test_force_list_callable_function():
    def force_list_func(path, key, value):
        return key == "server"

    xml = "<servers><server>test</server><item>other</item></servers>"
    result = xmltodict_rs.parse(xml, force_list=force_list_func)

    assert "servers" in result
    assert isinstance(result["servers"]["server"], list)
    assert result["servers"]["server"] == ["test"]
    assert isinstance(result["servers"]["item"], str)
    assert result["servers"]["item"] == "other"


def test_force_list_callable_with_path():
    def force_list_func(path, key, value):
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


def test_force_list_callable_errors():
    def force_list_func(path, key, value):
        raise ValueError("Test error from force_list function")

    xml = "<servers><server>test</server></servers>"

    with pytest.raises(ValueError, match="Test error from force_list function"):
        xmltodict_rs.parse(xml, force_list=force_list_func)


def test_force_list_invalid_types():
    xml = "<servers><server>test</server></servers>"

    class InvalidForceList:
        pass

    with pytest.raises(Exception):  # noqa: B017
        xmltodict_rs.parse(xml, force_list=InvalidForceList())


@pytest.mark.parametrize(
    "xml,force_list",
    [
        ("<root><item>1</item><item>2</item></root>", ("item",)),
        ("<config><servers><server>test</server></servers></config>", ("server",)),
    ],
)
def test_force_list_compatibility_with_original(xml, force_list):
    original = xmltodict.parse(xml, force_list=force_list)
    rust = xmltodict_rs.parse(xml, force_list=force_list)
    assert rust == original, f"Mismatch for XML: {xml}, force_list: {force_list}"


def test_force_list_true_makes_all_lists():
    xml = "<root><a>1</a><b>2</b></root>"
    result = xmltodict_rs.parse(xml, force_list=True)
    assert isinstance(result["root"]["a"], list)
    assert isinstance(result["root"]["b"], list)


def test_force_list_with_nested_elements():
    xml = """
    <root>
        <parent>
            <child>1</child>
        </parent>
        <parent>
            <child>2</child>
        </parent>
    </root>
    """
    result = xmltodict_rs.parse(xml, force_list=("child",))
    assert isinstance(result["root"]["parent"], list)
    assert isinstance(result["root"]["parent"][0]["child"], list)
    assert isinstance(result["root"]["parent"][1]["child"], list)


def test_force_list_empty_tuple():
    xml = "<root><item>1</item></root>"
    result = xmltodict_rs.parse(xml, force_list=())
    assert result["root"]["item"] == "1"


def test_force_list_with_attributes():
    xml = '<root><item id="1">value</item></root>'
    result = xmltodict_rs.parse(xml, force_list=("item",))
    assert isinstance(result["root"]["item"], list)
    assert result["root"]["item"][0]["@id"] == "1"
    assert result["root"]["item"][0]["#text"] == "value"
