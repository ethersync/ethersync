package io.github.ethersync

import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.diagnostic.logger
import com.intellij.openapi.project.Project
import com.intellij.util.io.awaitExit
import com.intellij.util.io.readLineAsync
import io.github.ethersync.protocol.EthersyncEditorProtocol
import io.github.ethersync.protocol.EthersyncEditorProtocolImpl
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.eclipse.lsp4j.jsonrpc.Launcher
import java.io.BufferedReader
import java.io.File
import java.io.InputStreamReader
import java.util.concurrent.Executors

private val LOG = logger<EthersyncServiceImpl>()

@Service(Service.Level.PROJECT)
class EthersyncServiceImpl(
   private val project: Project,
   private val cs: CoroutineScope
)  : EthersyncService {

   override fun connectToPeer(peer: String) {
      val projectDirectory = File(project.basePath!!)
      val ethersyncDirectory = File(projectDirectory, ".ethersync")
      val socket = "ethersync-%s-socket".format(project.name)

      cs.launch {
         if (!ethersyncDirectory.exists()) {
            LOG.debug("Creating ethersync directory")
            ethersyncDirectory.mkdir()
         }

         LOG.info("Starting ethersync daemon")
         val daemonProcessBuilder = ProcessBuilder("ethersync", "daemon", "--peer", peer, "--socket-name", socket)
            .directory(projectDirectory)
         val daemonProcess = daemonProcessBuilder.start()

         val notifier = project.messageBus.syncPublisher(DaemonOutputNotifier.CHANGE_ACTION_TOPIC)

         val stdout = BufferedReader(InputStreamReader(daemonProcess.inputStream))
         stdout.use {
            while (true) {
               val line = stdout.readLineAsync() ?: break;
               LOG.trace(line)
               notifier.logOutput(line)

               if (line.contains("Others can connect with")) {
                  launchEthersyncClient(socket, projectDirectory)
               }
            }
         }

         daemonProcess.awaitExit()
         if (daemonProcess.exitValue() != 0) {
            val stderr = BufferedReader(InputStreamReader(daemonProcess.errorStream))
            stderr.use {
               while (true) {
                  val line = stderr.readLineAsync() ?: break;
                  LOG.trace(line)
                  notifier.logOutput(line)
               }
            }

            notifier.logOutput("ethersync exited with exit code: " + daemonProcess.exitValue())
         }
      }
   }

   private fun launchEthersyncClient(socket: String, projectDirectory: File) {
      cs.launch {

         LOG.info("Starting ethersync client")
         val clientProcessBuilder = ProcessBuilder("ethersync", "client", "--socket-name", socket)
               .directory(projectDirectory)
         val clientProcess = clientProcessBuilder.start()

         val ethersyncEditorProtocol = project.service<EthersyncEditorProtocol>()
         val launcher = Launcher.createIoLauncher(
               ethersyncEditorProtocol,
               EthersyncEditorProtocol::class.java,
               clientProcess.inputStream,
               clientProcess.outputStream,
               Executors.newCachedThreadPool(),
               { c -> c },
               { gsonBuilder -> {} }
         )

         val listening = launcher.startListening()

         clientProcess.awaitExit()

         listening.cancel(true)

         if (clientProcess.exitValue() != 0) {
               val stderr = BufferedReader(InputStreamReader(clientProcess.errorStream))
               stderr.use {
                  while (true) {
                     val line = stderr.readLineAsync() ?: break;
                     LOG.trace(line)
                     System.out.println(line)
                  }
               }
         }
      }
   }
}
