# ooniprobe-rs

Rust components of OONI Probe. The `ooniprobe-passport` crate builds the
Passport library, which apps call through [UniFFI](https://mozilla.github.io/uniffi-rs/) bindings.

## Supported platforms

| Platform                       | Dependency                                                                              |
|--------------------------------|-----------------------------------------------------------------------------------------|
| Android                        | `org.ooni:passport-android` (Maven Central)                                             |
| Desktop: Linux, macOS, Windows | `org.ooni:passport-linux`, `org.ooni:passport-macos`, `org.ooni:passport-windows` (Maven Central) |
| iOS                            | Swift package published with each [GitHub release](https://github.com/ooni/ooniprobe-rs/releases) |

## Adding Passport to your app

### Android

The Kotlin bindings are in the `uniffi.ooniprobe` package.

```kotlin
dependencies {
    implementation("org.ooni:passport-android:<version>@aar")
}
```

### Desktop (JVM)

Add the artifact for the host OS, plus JNA. The bindings are in the
`uniffi.ooniprobe` package, as on Android:

```kotlin
dependencies {
    val os = org.gradle.internal.os.OperatingSystem.current()
    implementation(
        when {
            os.isMacOsX -> "org.ooni:passport-macos:<version>"
            os.isWindows -> "org.ooni:passport-windows:<version>"
            else -> "org.ooni:passport-linux:<version>"
        },
    )
}
```

### iOS (Swift Package Manager)

Each release publishes `Package.swift` and the Swift bindings `ooniprobe.swift`.
The manifest comes with the release and isn't in this repository, so add
Passport as a local Swift package:

1. Download both files of the release you want into a folder of your app
   repository:

   ```bash
   tag=v0.1.9   # or rolling
   base=https://github.com/ooni/ooniprobe-rs/releases/download/$tag
   mkdir -p Packages/Passport/Sources/Passport
   curl -fsSL -o Packages/Passport/Package.swift "$base/Package.swift"
   curl -fsSL -o Packages/Passport/Sources/Passport/ooniprobe.swift "$base/ooniprobe.swift"
   ```

   Swift Package Manager downloads `passport.xcframework.zip` from the URL in
   `Package.swift` and verifies its checksum.
2. In Xcode, select File > Add Package Dependencies > Add Local, choose the
   folder, and add the `Passport` library to your app target. From another
   package, use `.package(path: "Packages/Passport")`.
3. Commit the folder.

```swift
import Passport
```