# xmltodict_rs

High-performance XML to dict conversion library using Rust and PyO3

A Rust-based implementation of `xmltodict` that provides significant performance improvements while maintaining API compatibility.

## Features

-  **High Performance** - 5-10x faster than pure Python implementation
-  **Full Compatibility** - Drop-in replacement for `xmltodict`
-  **Type Safe** - Includes comprehensive type stubs (`.pyi` files) for better IDE support
-  **Safe** - Built with Rust for memory safety and security
-  **Easy to Use** - Simple installation and familiar API

## Versioning

The major and minor version numbers of `xmltodict_rs` match those of the original `xmltodict` library. This ensures that the behavior is consistent with the corresponding version of `xmltodict`, making it a true drop-in replacement. Patch versions may differ for Rust-specific fixes and optimizations.

For example:
- `xmltodict_rs 0.13.x` matches the behavior of `xmltodict 0.13.x`
- `xmltodict_rs 0.14.x` matches the behavior of `xmltodict 0.14.x`

## Installation

```bash
pip install xmltodict-rs
```

Or with uv:

```bash
uv add xmltodict-rs
```

If a pre-built wheel is not available for your platform, install the [Rust toolchain](https://rustup.rs/) first — the package will be compiled from source automatically.

## Quick Start

```python
import xmltodict_rs

# Parse XML to dictionary
xml = '<root><item id="1">value</item></root>'
result = xmltodict_rs.parse(xml)
print(result)
# {'root': {'item': {'@id': '1', '#text': 'value'}}}

# Convert dictionary back to XML
data = {'root': {'item': 'value'}}
xml = xmltodict_rs.unparse(data)
print(xml)
# <?xml version="1.0" encoding="utf-8"?>
# <root><item>value</item></root>
```

## Type Hints Support

Full type annotations are included for better IDE support and static type checking:

```python
from typing import Any
import xmltodict_rs

# IDE will provide autocomplete and type checking
result: dict[str, Any] = xmltodict_rs.parse("<root><item>test</item></root>")

# Type checkers like mypy will catch errors
xmltodict_rs.parse(123)  # Error: Expected str or bytes
```


## API Reference

### parse()

Convert XML to a Python dictionary.

```python
xmltodict_rs.parse(
    xml_input,                    # str or bytes: XML data to parse
    encoding=None,                # str: Only UTF-8 is supported; anything else raises NotImplementedError
    process_namespaces=False,     # bool: Process namespace prefixes
    namespace_separator=":",      # str: Separator for namespace and tag
    disable_entities=True,        # bool: Disable XML entities for security; False raises NotImplementedError
    process_comments=False,       # bool: Include XML comments in output
    xml_attribs=True,            # bool: Include attributes in output
    attr_prefix="@",             # str: Prefix for attribute keys
    cdata_key="#text",           # str: Key name for text content
    force_cdata=False,           # bool: Always wrap text in dict
    cdata_separator="",          # str: Separator for multiple text nodes
    strip_whitespace=True,       # bool: Remove whitespace-only text
    force_list=None,             # Control list creation
    postprocessor=None,          # Callback for transforming data
    item_depth=0,                # Streaming mode not implemented; >0 raises NotImplementedError
    item_callback=None,          # Streaming mode not implemented; non-None raises NotImplementedError
    comment_key="#comment",      # str: Key name for comments
    namespaces=None              # dict: Namespace URI mapping
)
```

### unparse()

Convert a Python dictionary back to XML.

```python
xmltodict_rs.unparse(
    input_dict,                   # dict: Dictionary to convert
    output=None,                  # file-like: Optional output object to write to
    encoding="utf-8",            # str: Character encoding
    full_document=True,          # bool: Include XML declaration
    short_empty_elements=False,  # bool: Use <tag/> for empty elements
    attr_prefix="@",             # str: Prefix identifying attributes
    cdata_key="#text",           # str: Key containing text content
    pretty=False,                # bool: Format with indentation
    newl="\n",                   # str: Newline character
    indent="\t",                 # str: Indentation string
    preprocessor=None            # Callback for transforming data
)
```

## Performance

Based on benchmarks with various XML sizes:

| Operation | Small (0.3KB) | Medium (15KB) | Large (150KB) |
|-----------|---------------|---------------|---------------|
| Parse     | ~8x faster    | ~6x faster    | ~5x faster    |
| Unparse   | ~10x faster   | ~11x faster   | ~10x faster   |

Measured with `benches/accurate_benchmark.py` against `xmltodict`; run-to-run
variance on this methodology is up to ~9% (see `docs/plans/` for raw
numbers), so treat the figures as an order-of-magnitude guide, not an SLA.

## Known Limitations

The following inputs are accepted by upstream `xmltodict` but are not
implemented in `xmltodict_rs`. Calling `parse()` with them raises
`NotImplementedError` instead of silently diverging from expected behavior
(all three limitations apply to `parse()` only — `unparse()` has no
`item_depth`/`item_callback`/`disable_entities` parameters, and its
`encoding` argument is written into the XML declaration as-is):

- **Streaming mode** — `item_depth > 0` or a non-`None` `item_callback` in
  `parse()`. Only whole-document parsing is supported.
- **`disable_entities=False`** in `parse()` — DTD entity expansion is not
  implemented; entities are always treated as disabled (the safe default).
- **Non-UTF-8 `encoding`** in `parse()` — only UTF-8 input is supported;
  passing any other `encoding` value raises `NotImplementedError`.

### Minor behavioral deviations

Found by differential audit against `xmltodict==0.13.0`; corner cases, not
`NotImplementedError`-gated, tracked for future alignment:

- `parse(strip_whitespace=False)` on an empty `<![CDATA[]]>` section returns
  `''`; upstream returns `None`.
- A `postprocessor` that returns `None` for the root element raises
  `ExpatError` ("no element found"); upstream returns `None`.
- `parse(process_namespaces=True, namespaces=...)` suppresses the `@xmlns`
  attribute on an element when every declared URI is covered by the
  `namespaces` mapping; upstream always emits `@xmlns` (with the raw,
  unmapped prefix -> URI pairs) whenever the element has other attributes
  and namespace declarations.
- `unparse(pretty=True)` whitespace layout can differ from upstream in
  mixed-content elements that combine `#text` with list-valued children
  (indentation/newline placement around the list, not the element content
  itself).

### Deep-nesting stack guard

The guard that lets `unparse()` handle arbitrarily deep dicts without
overflowing the thread stack (via [stacker](https://crates.io/crates/stacker))
is compiled only for x86_64 and aarch64 -- the platforms where its stack
switching is verified. On other targets (32-bit ARM, s390x, ppc64le)
extremely deep nesting may overflow the stack, as it did before 0.13.10.

## Development

### Setup

```bash
# Install dependencies
uv venv
uv sync

# Build the Rust extension
just dev

# Run tests
just test

# Run benchmarks
uv run python benches/accurate_benchmark.py
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Links

- [Original xmltodict](https://github.com/martinblech/xmltodict)
