MAX_WASM_BYTES := 65536
WASM := target/wasm32v1-none/optimized/blnt_interest_auction_filler.wasm
SOURCE_REPO_META := source_repo=github:BLNT-Protocol/blnt-interest-auction-filler

.PHONY: all build test check fmt verify-source-metadata wasm-size clean

all: check test build

build:
	mkdir -p target/wasm32v1-none/optimized
	stellar contract build --locked --package blnt-interest-auction-filler \
		--meta $(SOURCE_REPO_META) \
		--out-dir target/wasm32v1-none/optimized
	@size="$$(wc -c < "$(WASM)")"; \
		test "$$size" -le "$(MAX_WASM_BYTES)" || \
		{ printf 'error: optimized WASM exceeds %s-byte guard: %s\n' "$(MAX_WASM_BYTES)" "$$size" >&2; exit 1; }
	$(MAKE) --no-print-directory verify-source-metadata

test:
	cargo test --tests --locked

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features --locked -- -D warnings

fmt:
	cargo fmt --all

verify-source-metadata:
	stellar contract info meta --wasm "$(WASM)" 2>/dev/null | \
		grep -Fq "source_repo: github:BLNT-Protocol/blnt-interest-auction-filler"

wasm-size: build
	wc -c "$(WASM)"
	shasum -a 256 "$(WASM)"

clean:
	cargo clean
