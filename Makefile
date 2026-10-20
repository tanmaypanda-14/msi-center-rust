COMPOSE ?= docker compose
SHELL_DEV = $(COMPOSE) run --rm dev bash

VERSION ?= $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
DIST = dist/msi-center-$(VERSION)-x86_64-linux

.PHONY: build shell check test fmt clippy release run dist clean

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

dist: release      ## release tarball + checksum in ./dist
	mkdir -p $(DIST)
	cp target/release/msi-center $(DIST)/
	cp packaging/msi-center.desktop packaging/msi-center.svg $(DIST)/
	cp LICENSE README.md INSTALL.md $(DIST)/
	tar -C dist -czf $(DIST).tar.gz $(notdir $(DIST))
	cd dist && sha256sum $(notdir $(DIST)).tar.gz > $(notdir $(DIST)).tar.gz.sha256
	@ls -lh dist/

clean:
	$(SHELL_DEV) -c 'cargo clean'
