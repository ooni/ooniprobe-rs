plugins {
    kotlin("jvm") version "1.9.0"
}

group = "org.ooni"
version = "1.0-SNAPSHOT"

repositories {
    mavenCentral()
}

dependencies {
    implementation("net.java.dev.jna:jna:5.14.0")
    implementation("org.jetbrains.kotlin:kotlin-stdlib:1.9.0")
}

tasks.jar {
    val osName = project.findProperty("osName")?.toString() ?: "universal"

    archiveBaseName.set("passport")
    archiveAppendix.set(osName)
    archiveVersion.set("")
    
    from(sourceSets.main.get().output)
    
    manifest {
        attributes["Implementation-Title"] = "OONI Probe Passport"
        attributes["Implementation-Version"] = project.version
    }
}
