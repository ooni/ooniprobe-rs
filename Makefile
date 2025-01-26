build:
	cargo build
	cp target/debug/libooniprobe.dylib kotlin/lib/src/main/resources/libooniprobe.dylib
