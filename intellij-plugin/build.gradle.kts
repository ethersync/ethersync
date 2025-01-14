plugins {
   id("java")
   // Do not upgrade until following has been fixed:
   // https://github.com/kotlin-community-tools/kotlin-language-server/pull/17
   id("org.jetbrains.kotlin.jvm") version "2.0.21"
   id("org.jetbrains.intellij.platform") version "2.2.1"
}

group = "io.github.ethersync"
version = "0.7.0-SNAPSHOT"

repositories {
   mavenCentral()

   intellijPlatform {
      defaultRepositories()
   }
}

dependencies {
   intellijPlatform {
      intellijIdeaCommunity("2024.3.1.1")
      bundledPlugin("org.jetbrains.plugins.terminal")
   }

   implementation("org.eclipse.lsp4j:org.eclipse.lsp4j:0.23.1")
   implementation("org.eclipse.lsp4j:org.eclipse.lsp4j.jsonrpc:0.23.1")
}

kotlin {
   compilerOptions {
      jvmToolchain(17)
   }
}

tasks {
   patchPluginXml {
      sinceBuild.set("232")
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
