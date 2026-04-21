Pod::Spec.new do |s|
  s.name = "passport"
  s.version = "@RELEASE_VERSION@"
  s.summary = "OONI Probe Passport Library for iOS"
  s.author = "Mehul Gulati"
  s.homepage = "https://github.com/ooni/ooniprobe-rs"
  s.license = { :type => "https://opensource.org/licenses/BSD-3-Clause" }
  s.source = {
    :http => "https://repo1.maven.org/maven2/org/ooni/passport-ios/@VERSION@/passport-ios-@VERSION@.zip"
  }
  s.platform = :ios, "9.0"
  s.ios.vendored_frameworks = "passport.xcframework"
end
