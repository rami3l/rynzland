build: target/tla/tla2tools.jar
	java -Dtlc2.TLC.stopAfter=1800 -Dtlc2.TLC.ide=Github -cp $< tlc2.TLC -workers auto -lncheck final -checkpoint 60 -coverage 60 -tool -deadlock spec/RustupGC
.PHONY: build

target/tla/tla2tools.jar: target/tla
	wget -O $@ https://nightly.tlapl.us/dist/tla2tools.jar

target/tla:
	mkdir -p $@

clean:
	rm -r target/tla || true
.PHONY: clean
