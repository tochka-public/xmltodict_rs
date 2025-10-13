"""xmltodict_rs - High-performance XML to dict conversion library using Rust and PyO3.

This module provides parse() and unparse() functions for converting between
XML and Python dictionaries with better performance than pure Python implementations.
"""

from . import parse, unparse

__all__ = ["parse", "unparse"]
