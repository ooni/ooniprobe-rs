include:
	mkdir -p include/
	cd include && git clone git@github.com:ooni/userauth.git

update:
	cd include/userauth && git pull

clean:
	rm -rf include/

build:
	cargo build
	cp target/debug/libooniprobe.dylib kotlin/lib/src/main/resources/libooniprobe.dylib
