import typing as t

import xmltodict
import xmltodict_rs


def ref_unparse_preprocessor(key: str, value: t.Optional[t.Any]) -> tuple[str, t.Any]:
    if value is None:
        return key, None

    if isinstance(value, dict):
        value = {
            k: v
            for k, v in value.items()
            if not (isinstance(k, str) and k.startswith("@") and v is None)
        }

    return key, value


def ref_parse_postprocessor(path: list[t.Any], key: str, value: t.Any) -> tuple[str, t.Any]:
    if not isinstance(value, dict):
        return key, value

    for k, v in value.items():
        if (
            isinstance(k, str)
            and isinstance(v, str)
            and k.startswith("@")
            and ":nil" in k
            and k.endswith(":nil")
            and v.lower() == "true"
        ):
            return key, None

    return key, value


def _normalize(xml: str) -> str:
    return "".join(xml.split())


def test_nil_unparse_preprocessor_equivalence():
    input_dict = {
        "root": {
            "none_child": None,
            "attrs": {"@a": None, "@b": "1", "#text": "x"},
            "bool_true": True,
            "bool_false": False,
            "list": [None, {"@a": None, "@b": "2"}, "s"],
        }
    }

    xml_py = xmltodict.unparse(
        input_dict,
        full_document=False,
        short_empty_elements=False,
        preprocessor=ref_unparse_preprocessor,
    )

    xml_rs = xmltodict_rs.unparse(
        input_dict, full_document=False, short_empty_elements=False, preprocessor="nil"
    )

    assert _normalize(xml_py) == _normalize(xml_rs)


def test_nil_parse_postprocessor_equivalence():
    xml = (
        "<root>"
        '<a xsi:nil="true" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"></a>'
        '<b p3:nil="true" xmlns:p3="urn:foo"></b>'
        '<c xsi:nil="false" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">x</c>'
        "</root>"
    )

    py_parsed = xmltodict.parse(
        xml,
        process_namespaces=False,
        xml_attribs=True,
        postprocessor=ref_parse_postprocessor,
    )

    rs_parsed = xmltodict_rs.parse(
        xml,
        process_namespaces=False,
        xml_attribs=True,
        postprocessor="nil",
    )

    assert py_parsed == rs_parsed
