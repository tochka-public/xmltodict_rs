import sys

from .xmltodict_rs import *

_native = sys.modules[f"{__name__}.xmltodict_rs"]
__doc__ = _native.__doc__
if hasattr(_native, "__all__"):
    __all__ = _native.__all__
