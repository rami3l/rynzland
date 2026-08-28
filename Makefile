TLA_BASE := target/tla

TLA2TOOLS := $(TLA_BASE)/tla2tools.jar

spec: $(TLA2TOOLS)
	cd spec && \
		java -Dtlc2.TLC.stopAfter=1800 -Dtlc2.TLC.ide=Github -cp ../$< tlc2.TLC \
		-workers auto -lncheck final -checkpoint 60 -coverage 60 -tool -deadlock RustupGC
.PHONY: spec

$(TLA2TOOLS): | $(TLA_BASE)
	wget -O $@ https://nightly.tlapl.us/dist/tla2tools.jar

$(TLA_BASE):
	mkdir -p $@

APALACHE_VERSION := 0.47.0
APALACHE := $(TLA_BASE)/apalache/bin/apalache-mc

apalache: $(APALACHE)
	cd spec && ../$< check --config=RustupGC_apalache.cfg RustupGC.tla
.PHONY: apalache

apalache-simulate: $(APALACHE)
	cd spec && ../$< simulate --config=RustupGC_apalache.cfg RustupGC.tla
.PHONY: apalache-simulate

$(APALACHE): | $(TLA_BASE)
	wget https://github.com/apalache-mc/apalache/releases/download/v$(APALACHE_VERSION)/apalache.tgz \
		-O - | tar -xz -C $(TLA_BASE)

clean:
	rm -rf $(TLA_BASE)
	cd spec && git clean -fdX
 .PHONY: clean
