"""
Special test for whitespace handling in roundtrip scenarios
"""

import pytest
import xmltodict
import xmltodict_rs

def test_whitespace_roundtrip_with_strip_false():
    """Test that roundtrip works when strip_whitespace=False"""
    obj = {'text': '  spaces  '}

    # With strip_whitespace=False, roundtrip should preserve whitespace
    original_xml = xmltodict.unparse(obj)
    original_roundtrip = xmltodict.parse(original_xml, strip_whitespace=False)

    rust_xml = xmltodict_rs.unparse(obj)
    rust_roundtrip = xmltodict_rs.parse(rust_xml, strip_whitespace=False)

    # Both should preserve the original object
    assert rust_roundtrip == original_roundtrip == obj

def test_whitespace_default_behavior():
    """Test that both implementations have same default whitespace behavior"""
    obj = {'text': '  spaces  '}

    original_xml = xmltodict.unparse(obj)
    original_parsed = xmltodict.parse(original_xml)  # strip_whitespace=True by default

    rust_xml = xmltodict_rs.unparse(obj)
    rust_parsed = xmltodict_rs.parse(rust_xml)  # strip_whitespace=True by default

    # Both should strip whitespace by default
    assert original_parsed == rust_parsed == {'text': 'spaces'}
