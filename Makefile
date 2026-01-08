TOOLCHAINDIR := home/toolchain

all: $(TOOLCHAINDIR)/nightly-2024-01-01
.PHONY: all

clean:
	rm -r $(TOOLCHAINDIR)
.PHONY: clean

$(TOOLCHAINDIR)/%:
	cargo run -- $*
