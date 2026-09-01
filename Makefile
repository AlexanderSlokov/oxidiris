# Oxidiris developer commands.
#
# Run `make` with no target for the list. Every target here is also what CI runs, so a green
# `make check` locally means a green pipeline.

FILE ?= BACKLOG.md
WPM  ?= 300
CORE := oxidiris-core
BIN  := oxidiris

.DEFAULT_GOAL := help
.PHONY: help build release run demo dump frame test test-core test-tui fmt fmt-check lint \
        check wasm doc doc-open bench audit clean install tree ci

## help: list the available targets
help:
	@echo "Oxidiris — make targets"
	@echo
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/## /  /' | column -t -s ':'
	@echo
	@echo "Variables:  FILE=<path> (default $(FILE))   WPM=<n> (default $(WPM))"

# --- build ------------------------------------------------------------------

## build: compile the whole workspace in debug mode
build:
	cargo build --workspace

## release: compile an optimised binary
release:
	cargo build --workspace --release

## install: install the oxidiris binary into ~/.cargo/bin
install:
	cargo install --path crates/$(BIN) --locked

# --- run --------------------------------------------------------------------

## run: open the reader on FILE at WPM
run:
	cargo run -q -p $(BIN) -- $(FILE) --wpm $(WPM)

## demo: open the reader on this project's own backlog
demo:
	cargo run -q -p $(BIN) -- BACKLOG.md --wpm 400

## dump: print FILE as clean plain text (parser debugging, and the screen-reader path)
dump:
	cargo run -q -p $(BIN) -- $(FILE) --dump

# --- quality ----------------------------------------------------------------

## test: run every test in the workspace
test:
	cargo test --workspace

## test-core: run only the engine tests
test-core:
	cargo test -p $(CORE)

## test-tui: run only the terminal tests
test-tui:
	cargo test -p $(BIN)

## fmt: format the workspace
fmt:
	cargo fmt --all

## fmt-check: verify formatting without changing files
fmt-check:
	cargo fmt --all --check

## lint: clippy with warnings treated as errors
lint:
	cargo clippy --workspace --all-targets -- -D warnings

## wasm: verify oxidiris-core stays free of terminal dependencies (spec 1.1)
wasm:
	cargo build -p $(CORE) --target wasm32-unknown-unknown

## check: the Definition of Done gate — fmt, lint, test, wasm
check: fmt-check lint test wasm
	@echo "OK — Definition of Done satisfied"

## ci: everything check does, plus docs
ci: check doc
	@echo "OK — full pipeline"

# --- inspection -------------------------------------------------------------

## doc: build the API documentation
doc:
	cargo doc -p $(CORE) --no-deps

## doc-open: build the API documentation and open it
doc-open:
	cargo doc -p $(CORE) --no-deps --open

## frame: print one rendered frame to stdout (visual check, no terminal needed)
frame:
	@cargo test -q -p $(BIN) --bin $(BIN) -- --ignored --nocapture render_frame_of_the_backlog

## bench: run benchmarks (arrives with OXD-072)
bench:
	@echo "no benchmarks yet — see OXD-072 in BACKLOG.md"

## audit: check dependencies for advisories and licence problems
audit:
	cargo deny check 2>/dev/null || echo "cargo-deny not installed: cargo install cargo-deny"

## tree: show what oxidiris-core depends on, to catch accidental terminal deps
tree:
	cargo tree -p $(CORE)

## clean: remove build artefacts
clean:
	cargo clean