TLA_BASE := target/tla

TLA2TOOLS := $(TLA_BASE)/tla2tools.jar

spec: $(TLA2TOOLS)
	java -Dtlc2.TLC.stopAfter=1800 -Dtlc2.TLC.ide=Github -cp $< tlc2.TLC -workers auto -lncheck final -checkpoint 60 -coverage 60 -tool -deadlock spec/RustupGC
.PHONY: spec

$(TLA2TOOLS): | $(TLA_BASE)
	wget -O $@ https://nightly.tlapl.us/dist/tla2tools.jar

$(TLA_BASE):
	mkdir -p $@

APALACHE_VERSION := 0.47.0
APALACHE := $(TLA_BASE)/apalache/bin/apalache-mc

apalache: $(APALACHE)
	$< check --config=spec/RustupGC.cfg spec/RustupGC.tla
.PHONY: apalache

$(APALACHE): $(TLA_BASE)/apalache.tgz
	tar -xzf $< -C $(TLA_BASE)
	touch $@

$(TLA_BASE)/apalache.tgz: $(TLA_BASE)/apalache
	wget -O $@ https://github.com/apalache-mc/apalache/releases/download/v$(APALACHE_VERSION)/apalache.tgz

$(TLA_BASE)/apalache: | $(TLA_BASE)
	mkdir -p $@

clean:
	rm -rf $(TLA_BASE)
 .PHONY: clean
