package io.github.ethersync

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.ColoredProcessHandler
import com.intellij.execution.process.ProcessEvent
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
   private var daemonProcess: ColoredProcessHandler? = null
   private var clientProcess: Process? = null

   private val changetracker: Changetracker = Changetracker(project, cs)
   private val cursortracker: Cursortracker = Cursortracker(project, cs)

   init {
      val bus = project.messageBus.connect()
      bus.subscribe(FileEditorManagerListener.FILE_EDITOR_MANAGER, object : FileEditorManagerListener {
         override fun fileOpened(source: FileEditorManager, file: VirtualFile) {
            val canonicalFile = file.canonicalFile ?: return
            launchDocumentOpenRequest(canonicalFile.url)
         }

         override fun fileClosed(source: FileEditorManager, file: VirtualFile) {
            val canonicalFile = file.canonicalFile ?: return
            launchDocumentCloseNotification(canonicalFile.url)
         }
      })

      for (editor in FileEditorManager.getInstance(project).allEditors) {
         if (editor is TextEditor) {
            val file = editor.file ?: continue
            if (!file.exists()) {
               continue
            }
            editor.editor.caretModel.addCaretListener(cursortracker)
            editor.editor.document.addDocumentListener(changetracker)
         }
      }

      EditorFactory.getInstance().addEditorFactoryListener(object : EditorFactoryListener {
         override fun editorCreated(event: EditorFactoryEvent) {
            val file = event.editor.virtualFile ?: return
            if (!file.exists()) {
               return
            }

            event.editor.caretModel.addCaretListener(cursortracker)
            event.editor.document.addDocumentListener(changetracker)
         }

         override fun editorReleased(event: EditorFactoryEvent) {
            val file = event.editor.virtualFile ?: return
            if (!file.exists()) {
               return
            }

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
         it.process.destroy()
         it.process.awaitExit()
         daemonProcess = null
      }
      changetracker.clear()
      cursortracker.clear()
   }

   override fun start(peer: String?) {
      val socket = "ethersync-%s-socket".format(project.name)

      val cmd = GeneralCommandLine(AppSettings.getInstance().state.ethersyncBinaryPath)

      cmd.addParameter("daemon")
      peer?.let {
         if (peer.isNotBlank()) {
            cmd.addParameter("--peer")
            cmd.addParameter(peer)
         }
      }
      cmd.addParameter("--socket-name")
      cmd.addParameter(socket)

      launchDaemon(cmd)
   }

   override fun startWithCustomCommandLine(commandLine: String) {
      // TODO: splitting by " " is probably insufficient if there is an argument with spaces in it…
      val cmd = GeneralCommandLine(commandLine.split(" "))

      launchDaemon(cmd)
   }

   private fun launchDaemon(cmd: GeneralCommandLine) {
      val projectDirectory = File(project.basePath!!)
      val ethersyncDirectory = File(projectDirectory, ".ethersync")
      cmd.workDirectory = projectDirectory

      var socket: String? = null
      if (cmd.parametersList.hasParameter("--socket-name") || cmd.parametersList.hasParameter("-s")) {
         for (i in 0..(cmd.parametersList.parametersCount - 1)) {
            val name = cmd.parametersList[i]

            if (name == "--socket-name" || name == "-s") {
               socket = cmd.parametersList[i + 1]
               break
            }
         }
      }

      cs.launch {
         shutdown()

         if (!ethersyncDirectory.exists()) {
            LOG.debug("Creating ethersync directory")
            ethersyncDirectory.mkdir()
         }

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

   private fun launchEthersyncClient(socket: String?, projectDirectory: File) {
      if (clientProcess != null) {
         return
      }

      cs.launch {
         val cmd = GeneralCommandLine(AppSettings.getInstance().state.ethersyncBinaryPath)
         cmd.workDirectory = projectDirectory
         cmd.addParameter("client")

         socket?.let {
            cmd.addParameter("--socket-name")
            cmd.addParameter(it)
         }

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
