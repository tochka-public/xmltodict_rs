"""Property-based differential tests against reference xmltodict."""

import os

import hypothesis.strategies as st
import xmltodict
from hypothesis import given, settings

import xmltodict_rs

# QEMU-emulated CI targets (armv7 etc.) run examples orders of magnitude
# slower; select the reduced profile there via HYPOTHESIS_PROFILE.
settings.register_profile("default", max_examples=200, deadline=None)
settings.register_profile("ci-emulated", max_examples=10, deadline=None)
settings.load_profile(os.environ.get("HYPOTHESIS_PROFILE", "default"))

tag_names = st.from_regex(r"[a-z][a-z0-9_]{0,8}", fullmatch=True)
text_values = st.text(
    alphabet=st.characters(codec="utf-8", exclude_categories=("Cs", "Cc")),
    min_size=1,
    max_size=20,
)

xml_values = st.recursive(
    text_values | st.none(),
    lambda children: st.dictionaries(tag_names, children, min_size=1, max_size=4),
    max_leaves=15,
)


@given(tag=tag_names, value=xml_values)
def test_unparse_matches_reference(tag, value):
    doc = {tag: value}
    assert xmltodict_rs.unparse(doc) == xmltodict.unparse(doc)


@given(tag=tag_names, value=xml_values)
def test_roundtrip_matches_reference(tag, value):
    doc = {tag: value}
    xml = xmltodict.unparse(doc)
    assert xmltodict_rs.parse(xml) == xmltodict.parse(xml)
