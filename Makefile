CRATE := ooniprobe-ffi
UDL := ooniprobe-ffi/src/ooniprobe.udl
ANDROID_MODULE := android/ooniprobe
JNI_DIR := $(ANDROID_MODULE)/src/main/jniLibs
BINDINGS_DIR := $(ANDROID_MODULE)/src/main/java/org/ooni/ooniprobe

NDK_HOME ?= $(ANDROID_NDK_HOME)

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
	ANDROID_NDK_HOME=$(NDK_HOME) cargo ndk \
		-t armeabi-v7a \
		-t arm64-v8a \
		-t x86 \
		-t x86_64 \
		-o $(JNI_DIR) \
		build -p $(CRATE) --release

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
