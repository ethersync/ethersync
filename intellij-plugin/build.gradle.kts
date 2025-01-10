plugins {
   id("java")
   id("org.jetbrains.kotlin.jvm") version "1.9.25"
   id("org.jetbrains.intellij") version "1.17.4"
}

group = "io.github.ethersync"
version = "1.0-SNAPSHOT"

repositories {
   mavenCentral()
   maven {
      url = uri("https://jitpack.io")
   }
}

dependencies {
   implementation("com.github.Osiris-Team:jansi:2.4.6")
   implementation("org.eclipse.lsp4j:org.eclipse.lsp4j:0.23.1")
   implementation("org.eclipse.lsp4j:org.eclipse.lsp4j.jsonrpc:0.23.1")
   implementation("org.jsoup:jsoup:1.18.3")
}

// Configure Gradle IntelliJ Plugin
// Read more: https://plugins.jetbrains.com/docs/intellij/tools-gradle-intellij-plugin.html
intellij {
   version.set("2023.2.8")
   type.set("IC") // Target IDE Platform

   plugins.set(listOf("terminal"))
}

tasks {
   // Set the JVM compatibility versions
   withType<JavaCompile> {
      sourceCompatibility = "17"
      targetCompatibility = "17"
   }
   withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> {
      kotlinOptions.jvmTarget = "17"
   }

   patchPluginXml {
      sinceBuild.set("232")
      untilBuild.set("242.*")
   }

   signPlugin {
      certificateChain.set(System.getenv("CERTIFICATE_CHAIN"))
      privateKey.set(System.getenv("PRIVATE_KEY"))
      password.set(System.getenv("PRIVATE_KEY_PASSWORD"))
   }

   publishPlugin {
      token.set(System.getenv("PUBLISH_TOKEN"))
   }
}
