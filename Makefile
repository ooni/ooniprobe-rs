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

DESKTOP_DIR := desktop
DESKTOP_RESOURCES := $(DESKTOP_DIR)/src/main/resources
DESKTOP_BINDINGS_DIR := $(DESKTOP_DIR)/src/main/kotlin

MACOS_TARGETS := \
	aarch64-apple-darwin \
	x86_64-apple-darwin

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
		target/aarch64-apple-ios-sim/release/libuniffi_ooniprobe.a \
		target/x86_64-apple-ios/release/libuniffi_ooniprobe.a \
		-output target/ios-simulator-universal/release/libuniffi_ooniprobe.a

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
		-library target/aarch64-apple-ios/release/libuniffi_ooniprobe.a -headers $(SWIFT_DIR) \
		-library target/ios-simulator-universal/release/libuniffi_ooniprobe.a -headers $(SWIFT_DIR) \
		-output $(XCFRAMEWORK_DIR)

.PHONY: ios
ios: ios-xcframework

.PHONY: clean-desktop
clean:
	cargo clean -p $(CRATE)
	rm -rf $(DESKTOP_RESOURCES)
	rm -rf $(DESKTOP_BINDINGS_DIR)

.PHONY: desktop-bindings
desktop-bindings:
	@mkdir -p $(DESKTOP_BINDINGS_DIR)
	cargo run -p uniffi-bindgen -- \
		generate $(UDL) \
		--language kotlin \
		--out-dir $(DESKTOP_BINDINGS_DIR)

.PHONY: linux/x86_64
linux/x86:
	rustup target add x86_64-unknown-linux-gnu
	cargo build -p $(CRATE) --target x86_64-unknown-linux-gnu --release

.PHONY: linux/aarch64
linux/aarch64:
	rustup target add aarch64-unknown-linux-gnu 
	cargo build -p $(CRATE) --target aarch64-unknown-linux-gnu --release	

.PHONY: desktop-linux
desktop-linux:
	@mkdir -p $(DESKTOP_RESOURCES)/linux-x86-64
	cp target/x86_64-unknown-linux-gnu/release/libuniffi_ooniprobe.so $(DESKTOP_RESOURCES)/linux-x86-64/

	@mkdir -p $(DESKTOP_RESOURCES)/linux-aarch64
	cp target/aarch64-unknown-linux-gnu/release/libuniffi_ooniprobe.so $(DESKTOP_RESOURCES)/linux-aarch64/

	$(MAKE) desktop-jar OS_NAME=linux

.PHONY: macos-targets
macos-targets:
	@for t in $(MACOS_TARGETS); do \
		rustup target add $$t; \
	done

.PHONY: macos-libs
macos-libs: macos-targets
	cargo build -p $(CRATE) --target aarch64-apple-darwin --release
	cargo build -p $(CRATE) --target x86_64-apple-darwin --release	

.PHONY: desktop-macos
desktop-macos: macos-libs
	@mkdir -p $(DESKTOP_RESOURCES)/darwin-universal
	lipo -create \
		target/aarch64-apple-darwin/release/libuniffi_ooniprobe.dylib \
		target/x86_64-apple-darwin/release/libuniffi_ooniprobe.dylib \
		-output $(DESKTOP_RESOURCES)/darwin-universal/libuniffi_ooniprobe.dylib

	$(MAKE) desktop-jar OS_NAME=macos

.PHONY: desktop-windows
desktop-windows:
	cargo build -p $(CRATE) --release
	@mkdir -p $(DESKTOP_RESOURCES)/win32-x86-64
	cp target/release/uniffi_ooniprobe.dll $(DESKTOP_RESOURCES)/win32-x86-64/
	$(MAKE) desktop-jar OS_NAME=windows

.PHONY: desktop-jar
desktop-jar: desktop-bindings
	cd $(DESKTOP_DIR) && ./gradlew jar -PosName=$(OS_NAME)
