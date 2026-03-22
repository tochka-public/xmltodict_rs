from . import xmltodict_rs as _native
from .xmltodict_rs import *

__doc__ = _native.__doc__
if hasattr(_native, "__all__"):
    __all__ = _native.__all__
