CARGO = /usr/bin/cargo

debug:
	cargo build

test:
	cargo test

release:
	cargo build --release

clean:
	rm -rf target

examples:
	cd examples && $(MAKE)

.PHONY: debug test release clean examples
