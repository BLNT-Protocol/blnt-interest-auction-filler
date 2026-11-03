MAX_WASM_BYTES := 65536
WASM := target/wasm32v1-none/optimized/blnt_interest_auction_filler.wasm
SOURCE_REPO_META := source_repo=github:BLNT-Protocol/blnt-interest-auction-filler

.PHONY: all build test check fmt verify-source-metadata wasm-size release-verify mainnet-plan mainnet-deploy mainnet-verify clean

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

release-verify:
	bash scripts/deploy.sh release-verify

mainnet-plan:
	bash scripts/deploy.sh plan

mainnet-deploy:
	bash scripts/deploy.sh deploy

mainnet-verify:
	@test -n "$(BLNT_FILLER_CONTRACT)" || \
		{ printf 'error: BLNT_FILLER_CONTRACT is required\n' >&2; exit 1; }
	bash scripts/deploy.sh verify "$(BLNT_FILLER_CONTRACT)"

clean:
	cargo clean
