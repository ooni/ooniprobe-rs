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

XCFRAMEWORK_DIR := ios/ooniProbe.xcframework
SWIFT_DIR := ios/Sources/OoniProbe

IPHONEOS_SDK = $(shell xcrun --sdk iphoneos --show-sdk-path)
IPHONESIMULATOR_SDK = $(shell xcrun --sdk iphonesimulator --show-sdk-path)
MIN_IOS_VERSION := 13.0

IOS_TARGETS := \
	aarch64-apple-ios \
	aarch64-apple-ios-sim \
	x86_64-apple-ios

.PHONY: help
help:
	@echo "Available targets:"
	@echo "  make android        Build Rust + Kotlin + AAR"
	@echo "  make android-so     Build Android .so only"
	@echo "  make bindings       Generate Kotlin bindings"
	@echo "  make aar            Build Android AAR"
	@echo "  make android-clean  Clean android builds"

.PHONY: clean-android
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

.PHONY: ios-targets
ios-targets:
	@for t in $(IOS_TARGETS); do \
		rustup target add $$t; \
	done

.PHONY: ios-libs
ios-libs: ios-targets
	SDKROOT=$(IPHONEOS_SDK) \
	IPHONEOS_DEPLOYMENT_TARGET=$(MIN_IOS_VERSION) \
	cargo build -p $(CRATE) --target aarch64-apple-ios --release

	SDKROOT=$(IPHONESIMULATOR_SDK) \
	IPHONEOS_DEPLOYMENT_TARGET=$(MIN_IOS_VERSION) \
	BINDGEN_EXTRA_CLANG_ARGS="-target arm64-apple-ios$(MIN_IOS_VERSION)-simulator" \
	cargo build -p $(CRATE) --target aarch64-apple-ios-sim --release

	SDKROOT=$(IPHONESIMULATOR_SDK) \
	IPHONEOS_DEPLOYMENT_TARGET=$(MIN_IOS_VERSION) \
	BINDGEN_EXTRA_CLANG_ARGS="-target x86_64-apple-ios$(MIN_IOS_VERSION)-simulator" \
	cargo build -p $(CRATE) --target x86_64-apple-ios --release

.PHONY: ios-universal-sim
ios-universal-sim: ios-libs
	@mkdir -p target/ios-simulator-universal/release
	lipo -create \
		target/aarch64-apple-ios-sim/release/libooniprobe_ffi.a \
		target/x86_64-apple-ios/release/libooniprobe_ffi.a \
		-output target/ios-simulator-universal/release/libooniprobe_ffi.a

.PHONY: ios-bindings
ios-bindings:
	@mkdir -p $(SWIFT_DIR)
	cargo run -p uniffi-bindgen -- \
		generate $(UDL) \
		--language swift \
		--out-dir $(SWIFT_DIR)

.PHONY: ios-xcframework
ios-xcframework: ios-universal-sim ios-bindings
	@rm -rf $(XCFRAMEWORK_DIR)
	cp $(SWIFT_DIR)/ooniprobeFFI.modulemap $(SWIFT_DIR)/module.modulemap
	
	xcodebuild -create-xcframework \
		-library target/aarch64-apple-ios/release/libooniprobe_ffi.a -headers $(SWIFT_DIR) \
		-library target/ios-simulator-universal/release/libooniprobe_ffi.a -headers $(SWIFT_DIR) \
		-output $(XCFRAMEWORK_DIR)

.PHONY: ios
ios: ios-xcframework
