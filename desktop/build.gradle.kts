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

    archiveBaseName.set("ooniprobe-desktop")
    archiveAppendix.set(osName)
    archiveVersion.set("")
    
    from(sourceSets.main.get().output)
    
    manifest {
        attributes["Implementation-Title"] = "OoniProbe Desktop"
        attributes["Implementation-Version"] = project.version
    }
}
