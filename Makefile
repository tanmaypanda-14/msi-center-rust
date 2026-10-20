COMPOSE ?= docker compose
SHELL_DEV = $(COMPOSE) run --rm dev bash

.PHONY: build shell check test fmt clippy release run clean

build:            ## build the dev image
	$(COMPOSE) build dev

shell:            ## interactive shell in the container
	$(SHELL_DEV)

check:            ## type-check the workspace
	$(SHELL_DEV) -c 'cargo check --workspace --all-targets'

test:             ## run tests
	$(SHELL_DEV) -c 'cargo test --workspace'

fmt:              ## format
	$(SHELL_DEV) -c 'cargo fmt --all'

clippy:           ## lints
	$(SHELL_DEV) -c 'cargo clippy --workspace --all-targets -- -D warnings'

release:          ## optimised binary -> ./target/release/msi-center
	$(SHELL_DEV) -c 'cargo build --release --workspace'
	@ls -lh target/release/msi-center

run: release      ## build in the container, run on the host (needs the EC + a session)
	./target/release/msi-center

clean:
	$(SHELL_DEV) -c 'cargo clean'
