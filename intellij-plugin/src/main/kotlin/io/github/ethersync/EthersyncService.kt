package io.github.ethersync

interface EthersyncService {

   fun start(peer: String?)

   fun startWithCustomCommandLine(commandLine: String)
}
