package io.github.ethersync

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.ColoredProcessHandler
import com.intellij.execution.process.ProcessEvent
import com.intellij.execution.process.ProcessHandler
import com.intellij.execution.process.ProcessListener
import com.intellij.execution.ui.ConsoleView
import com.intellij.openapi.components.Service
import com.intellij.openapi.diagnostic.logger
import com.intellij.openapi.editor.EditorFactory
import com.intellij.openapi.editor.event.*
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.fileEditor.TextEditor
import com.intellij.openapi.project.Project
import com.intellij.openapi.project.ProjectManager
import com.intellij.openapi.project.ProjectManagerListener
import com.intellij.openapi.rd.util.withUiContext
import com.intellij.openapi.util.Key
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.openapi.wm.ToolWindowManager
import com.intellij.util.io.await
import com.intellij.util.io.awaitExit
import com.intellij.util.io.readLineAsync
import io.github.ethersync.protocol.*
import io.github.ethersync.settings.AppSettings
import io.github.ethersync.sync.Changetracker
import io.github.ethersync.sync.Cursortracker
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.eclipse.lsp4j.jsonrpc.Launcher
import org.eclipse.lsp4j.jsonrpc.ResponseErrorException
import java.io.BufferedReader
import java.io.File
import java.io.InputStreamReader
import java.util.concurrent.Executors

private val LOG = logger<EthersyncServiceImpl>()

@Service(Service.Level.PROJECT)
class EthersyncServiceImpl(
   private val project: Project,
   private val cs: CoroutineScope,
)  : EthersyncService {

   private var launcher: Launcher<RemoteEthersyncClientProtocol>? = null
   private var daemonProcess: ProcessHandler? = null
   private var clientProcess: Process? = null

   private val changetracker: Changetracker = Changetracker(project, cs)
   private val cursortracker: Cursortracker = Cursortracker(project, cs)

   init {
      val bus = project.messageBus.connect()
      bus.subscribe(FileEditorManagerListener.FILE_EDITOR_MANAGER, object : FileEditorManagerListener {
         override fun fileOpened(source: FileEditorManager, file: VirtualFile) {
            launchDocumentOpenRequest(file.canonicalFile!!.url)
         }

         override fun fileClosed(source: FileEditorManager, file: VirtualFile) {
            launchDocumentCloseNotification(file.canonicalFile!!.url)
         }
      })

      for (editor in FileEditorManager.getInstance(project).allEditors) {
         if (editor is TextEditor) {
            editor.editor.caretModel.addCaretListener(cursortracker)
            editor.editor.document.addDocumentListener(changetracker)
         }
      }

      EditorFactory.getInstance().addEditorFactoryListener(object : EditorFactoryListener {
         override fun editorCreated(event: EditorFactoryEvent) {
            event.editor.caretModel.addCaretListener(cursortracker)
            event.editor.document.addDocumentListener(changetracker)
         }

         override fun editorReleased(event: EditorFactoryEvent) {
            event.editor.caretModel.removeCaretListener(cursortracker)
            event.editor.document.removeDocumentListener(changetracker)
         }
      }, project)

      ProjectManager.getInstance().addProjectManagerListener(project, object: ProjectManagerListener {
         override fun projectClosingBeforeSave(project: Project) {
            cs.launch {
               shutdown()
            }
         }
      })
   }

   suspend fun shutdown() {
      clientProcess?.let {
         it.destroy()
         it.awaitExit()
         clientProcess = null
      }
      daemonProcess?.let {
         it.detachProcess()
         it.destroyProcess()
         daemonProcess = null
      }
      changetracker.clear()
      cursortracker.clear()
   }

   override fun connectToPeer(peer: String) {
      val projectDirectory = File(project.basePath!!)
      val ethersyncDirectory = File(projectDirectory, ".ethersync")
      val socket = "ethersync-%s-socket".format(project.name)

      if (!ethersyncDirectory.exists()) {
         LOG.debug("Creating ethersync directory")
         ethersyncDirectory.mkdir()
      }

      val cmd = GeneralCommandLine(AppSettings.getInstance().state.ethersyncBinaryPath)
      cmd.workDirectory = projectDirectory
      cmd.addParameter("daemon")
      if (peer.isNotBlank()) {
         cmd.addParameter("--peer")
         cmd.addParameter(peer)
      }
      cmd.addParameter("--socket-name")
      cmd.addParameter(socket)

      cs.launch {
         shutdown()

         withUiContext {
            daemonProcess = ColoredProcessHandler(cmd)

            daemonProcess!!.addProcessListener(object : ProcessListener {
               override fun onTextAvailable(event: ProcessEvent, outputType: Key<*>) {
                  if (event.text.contains("Others can connect with")) {
                     launchEthersyncClient(socket, projectDirectory)
                  }
               }

               override fun processTerminated(event: ProcessEvent) {
                  cs.launch {
                     shutdown()
                  }
               }
            })


            val tw = ToolWindowManager.getInstance(project).getToolWindow("ethersync")!!
            val console =  tw.contentManager.findContent("Daemon")!!.component
            if (console is ConsoleView) {
               console.attachToProcess(daemonProcess!!)
            }

            tw.show()

            daemonProcess!!.startNotify()
         }

      }
   }

   private fun createProtocolHandler(): EthersyncEditorProtocol {

      return object : EthersyncEditorProtocol {
         override fun cursor(cursorEvent: CursorEvent) {
            cursortracker.handleRemoteCursorEvent(cursorEvent)
         }

         override fun edit(editEvent: EditEvent) {
            changetracker.handleRemoteEditEvent(editEvent)
         }

      }
   }

   private fun launchEthersyncClient(socket: String, projectDirectory: File) {
      if (clientProcess != null) {
         return
      }

      cs.launch {
         LOG.info("Starting ethersync client")
         // TODO: try catch not existing binary
         val clientProcessBuilder = ProcessBuilder(
            AppSettings.getInstance().state.ethersyncBinaryPath,
            "client", "--socket-name", socket)
               .directory(projectDirectory)
         clientProcess = clientProcessBuilder.start()
         val clientProcess = clientProcess!!

         val ethersyncEditorProtocol = createProtocolHandler()
         launcher = Launcher.createIoLauncher(
               ethersyncEditorProtocol,
               RemoteEthersyncClientProtocol::class.java,
               clientProcess.inputStream,
               clientProcess.outputStream,
               Executors.newCachedThreadPool(),
               { c -> c },
               { _ -> run {} }
         )

         val listening = launcher!!.startListening()
         cursortracker.remoteProxy = launcher!!.remoteProxy
         changetracker.remoteProxy = launcher!!.remoteProxy

         val fileEditorManager = FileEditorManager.getInstance(project)
         for (file in fileEditorManager.openFiles) {
            launchDocumentOpenRequest(file.canonicalFile!!.url)
         }

         clientProcess.awaitExit()

         listening.cancel(true)
         listening.await()

         if (clientProcess.exitValue() != 0) {
            val stderr = BufferedReader(InputStreamReader(clientProcess.errorStream))
            stderr.use {
               while (true) {
                  val line = stderr.readLineAsync() ?: break;
                  LOG.trace(line)
               }
            }
         }
      }
   }

   fun launchDocumentCloseNotification(fileUri: String) {
      val launcher = launcher ?: return
      cs.launch {
         launcher.remoteProxy.close(DocumentRequest(fileUri))
         changetracker.closeFile(fileUri)
      }
   }

   fun launchDocumentOpenRequest(fileUri: String) {
      val launcher = launcher ?: return
      cs.launch {
         try {
            changetracker.openFile(fileUri)
            launcher.remoteProxy.open(DocumentRequest(fileUri)).await()
         } catch (e: ResponseErrorException) {
            TODO("not yet implemented: notify about an protocol error")
         }
      }
   }

}
