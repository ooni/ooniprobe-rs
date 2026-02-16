CRATE := ooniprobe-ffi
UDL := ooniprobe-ffi/src/ooniprobe.udl
ANDROID_MODULE := android/ooniprobe
JNI_DIR := $(ANDROID_MODULE)/src/main/jniLibs
BINDINGS_DIR := $(ANDROID_MODULE)/src/main/java/org/ooni/ooniprobe

ANDROID_TARGETS := \
	aarch64-linux-android \
	armv7-linux-androideabi \
	i686-linux-android \
	x86_64-linux-android

.PHONY: help
help:
	@echo "Available targets:"
	@echo "  make android        Build Rust + Kotlin + AAR"
	@echo "  make android-so     Build Android .so only"
	@echo "  make bindings       Generate Kotlin bindings"
	@echo "  make aar            Build Android AAR"
	@echo "  make clean          Clean everything"

.PHONY: clean
clean:
	cargo clean -p $(CRATE)
	rm -rf $(JNI_DIR)
	rm -rf $(BINDINGS_DIR)

.PHONY: android-targets
android-targets:
	@for t in $(ANDROID_TARGETS); do \
		rustup target add $$t; \
	done


.PHONY: android-so
android-so: android-targets
	@for t in $(ANDROID_TARGETS); do \
		cargo build -p $(CRATE) --target $$t --release; \
	done

	rm -rf $(JNI_DIR)
	mkdir -p $(JNI_DIR)/arm64-v8a
	mkdir -p $(JNI_DIR)/armeabi-v7a
	mkdir -p $(JNI_DIR)/x86_64

	cp target/aarch64-linux-android/release/libooniprobe_ffi.so \
		$(JNI_DIR)/arm64-v8a/

	cp target/armv7-linux-androideabi/release/libooniprobe_ffi.so \
		$(JNI_DIR)/armeabi-v7a/

	cp target/x86_64-linux-android/release/libooniprobe_ffi.so \
		$(JNI_DIR)/x86_64/

.PHONY: bindings
bindings:
	mkdir -p $(BINDINGS_DIR)
	cargo run -p uniffi-bindgen -- \
		generate $(UDL) \
		--language kotlin \
		--out-dir $(BINDINGS_DIR)

.PHONY: aar
aar:
	cd android && ./gradlew :ooniprobe:assembleRelease

.PHONY: android
android: android-so bindings aar

.PHONY: publish
publish:
	cd android && ./gradlew :ooniprobe:publish
