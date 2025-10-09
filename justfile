default: check

lint-py:
    uv run ruff check . --fix

lint-rust:
    cargo clippy --all-targets --all-features -- -D warnings -W clippy::style -D clippy::perf

lint: lint-py lint-rust

clippy: lint

fmt-py:
    uv run ruff format --exit-non-zero-on-format .

fmt-rust:
    cargo fmt

fmt: fmt-py fmt-rust

check: fmt lint

dev:
    uv cache clean
    uv venv --allow-existing
    uv run maturin develop --release

test: dev
    uv run pytest tests/ -v

build:
    uv run maturin build --release

clean:
    cargo clean
    rm -rf dist/
    rm -rf target/wheels/
    uv cache clean
    uv venv --clear
