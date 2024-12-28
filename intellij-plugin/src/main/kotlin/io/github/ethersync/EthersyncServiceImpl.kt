package io.github.ethersync

import com.google.gson.JsonObject
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.application.ModalityState
import com.intellij.openapi.components.Service
import com.intellij.openapi.diagnostic.logger
import com.intellij.openapi.editor.EditorFactory
import com.intellij.openapi.editor.LogicalPosition
import com.intellij.openapi.editor.event.CaretEvent
import com.intellij.openapi.editor.event.CaretListener
import com.intellij.openapi.editor.event.EditorFactoryEvent
import com.intellij.openapi.editor.event.EditorFactoryListener
import com.intellij.openapi.editor.markup.HighlighterLayer
import com.intellij.openapi.editor.markup.HighlighterTargetArea
import com.intellij.openapi.editor.markup.RangeHighlighter
import com.intellij.openapi.editor.markup.TextAttributes
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.fileEditor.TextEditor
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.testFramework.utils.editor.getVirtualFile
import com.intellij.ui.JBColor
import com.intellij.util.io.awaitExit
import com.intellij.util.io.readLineAsync
import io.github.ethersync.protocol.CursorEvent
import io.github.ethersync.protocol.CursorRequest
import io.github.ethersync.protocol.DocumentRequest
import io.github.ethersync.protocol.EthersyncEditorProtocol
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.eclipse.lsp4j.Position
import org.eclipse.lsp4j.Range
import org.eclipse.lsp4j.jsonrpc.Launcher
import java.io.BufferedReader
import java.io.File
import java.io.InputStreamReader
import java.util.Collections
import java.util.LinkedList
import java.util.concurrent.CompletableFuture
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

   private fun createProtocolHandler(): EthersyncEditorProtocol {
      val highlighter = LinkedList<RangeHighlighter>()

      return object : EthersyncEditorProtocol {
         override fun cursor(cursorEvent: CursorEvent) {
            val fileEditorManager = FileEditorManager.getInstance(project)

            val fileEditor = fileEditorManager.allEditors
               .first { editor -> editor.file.url == cursorEvent.documentUri } ?: return

            if (fileEditor is TextEditor) {
               val editor = fileEditor.editor
               ApplicationManager.getApplication().invokeLater({
                  synchronized(highlighter) {
                     val markupModel = editor.markupModel

                     for (hl in highlighter) {
                        markupModel.removeHighlighter(hl)
                     }
                     highlighter.clear()

                     for(range in cursorEvent.ranges) {
                        val startPosition = editor.logicalPositionToOffset(LogicalPosition(range.start.line, range.start.character))
                        val endPosition = editor.logicalPositionToOffset(LogicalPosition(range.end.line, range.end.character))

                        val textAttributes = TextAttributes().apply {
                           backgroundColor = JBColor(JBColor.YELLOW, JBColor.DARK_GRAY)
                           // TODO: unclear which is the best effect type
                           // effectType = EffectType.LINE_UNDERSCORE
                           // effectColor = JBColor(JBColor.YELLOW, JBColor.DARK_GRAY)
                        }

                        val hl = markupModel.addRangeHighlighter(
                           startPosition,
                           endPosition + 1,
                           HighlighterLayer.ADDITIONAL_SYNTAX,
                           textAttributes,
                           HighlighterTargetArea.EXACT_RANGE
                        )

                        highlighter.add(hl)
                     }
                  }
               }, ModalityState.nonModal())
            }
         }

         override fun open(documentRequest: DocumentRequest): CompletableFuture<JsonObject> {
            return CompletableFuture.completedFuture(JsonObject())
         }

         override fun close(documentRequest: DocumentRequest): CompletableFuture<JsonObject> {
            return CompletableFuture.completedFuture(JsonObject())
         }

      }
   }

   private fun launchEthersyncClient(socket: String, projectDirectory: File) {

      cs.launch {

         LOG.info("Starting ethersync client")
         val clientProcessBuilder = ProcessBuilder("ethersync", "client", "--socket-name", socket)
               .directory(projectDirectory)
         val clientProcess = clientProcessBuilder.start()

         val ethersyncEditorProtocol = createProtocolHandler()
         val launcher = Launcher.createIoLauncher(
               ethersyncEditorProtocol,
               EthersyncEditorProtocol::class.java,
               clientProcess.inputStream,
               clientProcess.outputStream,
               Executors.newCachedThreadPool(),
               { c -> c },
               { _ -> run {} }
         )

         val bus = project.messageBus.connect()
         bus.subscribe(FileEditorManagerListener.FILE_EDITOR_MANAGER, object : FileEditorManagerListener {
            override fun fileOpened(source: FileEditorManager, file: VirtualFile) {
               ethersyncEditorProtocol.open(DocumentRequest(file.url))
            }

            override fun fileClosed(source: FileEditorManager, file: VirtualFile) {
               ethersyncEditorProtocol.close(DocumentRequest(file.url))
            }
         })

         val caretListener = object : CaretListener {
            override fun caretPositionChanged(event: CaretEvent) {
               val uri = event.editor.virtualFile.url
               val pos = Position(event.newPosition.line, event.newPosition.column)
               val range = Range(pos, pos)
               launcher.remoteEndpoint.notify("cursor", CursorRequest(uri, Collections.singletonList(range)))
            }
         }

         EditorFactory.getInstance().addEditorFactoryListener(object : EditorFactoryListener {
            override fun editorCreated(event: EditorFactoryEvent) {
               event.editor.caretModel.addCaretListener(caretListener)
            }

            override fun editorReleased(event: EditorFactoryEvent) {
               event.editor.caretModel.removeCaretListener(caretListener)
            }
         }, project)


         val listening = launcher.startListening()

         cs.launch {
            val fileEditorManager = FileEditorManager.getInstance(project)

            for (file in fileEditorManager.openFiles) {
               ethersyncEditorProtocol.open(DocumentRequest(file.url))
            }
         }

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
