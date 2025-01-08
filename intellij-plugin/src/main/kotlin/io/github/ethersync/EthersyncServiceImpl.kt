package io.github.ethersync

import com.intellij.openapi.application.EDT
import com.intellij.openapi.command.WriteCommandAction
import com.intellij.openapi.components.Service
import com.intellij.openapi.diagnostic.logger
import com.intellij.openapi.editor.EditorFactory
import com.intellij.openapi.editor.LogicalPosition
import com.intellij.openapi.editor.event.*
import com.intellij.openapi.editor.markup.*
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.fileEditor.TextEditor
import com.intellij.openapi.project.Project
import com.intellij.openapi.project.ProjectManager
import com.intellij.openapi.project.ProjectManagerListener
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.refactoring.suggested.newRange
import com.intellij.ui.JBColor
import com.intellij.util.io.await
import com.intellij.util.io.awaitExit
import com.intellij.util.io.readLineAsync
import io.github.ethersync.protocol.*
import io.github.ethersync.settings.AppSettings
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.eclipse.lsp4j.Position
import org.eclipse.lsp4j.Range
import org.eclipse.lsp4j.jsonrpc.Launcher
import org.eclipse.lsp4j.jsonrpc.ResponseErrorException
import java.io.BufferedReader
import java.io.File
import java.io.IOException
import java.io.InputStreamReader
import java.util.*
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

private val LOG = logger<EthersyncServiceImpl>()

@Service(Service.Level.PROJECT)
class EthersyncServiceImpl(
   private val project: Project,
   private val cs: CoroutineScope,
)  : EthersyncService {

   private var launcher: Launcher<RemoteEthersyncClientProtocol>? = null
   private var daemonProcess: Process? = null
   private var clientProcess: Process? = null

   private val ignoreChangeEvent = AtomicBoolean(false)

   data class FileRevision(
      // Number of operations the daemon has made.
      var daemon: UInt = 0u,
      // Number of operations we have made.
      var editor: UInt = 0u,
   )
   val revisions: HashMap<String, FileRevision> = HashMap()

   init {
      val bus = project.messageBus.connect()
      bus.subscribe(FileEditorManagerListener.FILE_EDITOR_MANAGER, object : FileEditorManagerListener {
         override fun fileOpened(source: FileEditorManager, file: VirtualFile) {
            launchDocumentOpenRequest(file.url)
         }

         override fun fileClosed(source: FileEditorManager, file: VirtualFile) {
            launchDocumentCloseNotification(file.url)
         }
      })

      val caretListener = object : CaretListener {
         override fun caretPositionChanged(event: CaretEvent) {
            val uri = event.editor.virtualFile.url
            val pos = Position(event.newPosition.line, event.newPosition.column)
            val range = Range(pos, pos)
            launchCursorRequest(CursorRequest(uri, Collections.singletonList(range)))
         }
      }

      val documentListener = object : DocumentListener {
         override fun documentChanged(event: DocumentEvent) {
            if (ignoreChangeEvent.get()) {
               return
            }

            val file = FileDocumentManager.getInstance().getFile(event.document)!!
            val fileEditor = FileEditorManager.getInstance(project).getEditors(file)
               .filterIsInstance<TextEditor>()
               .first()

            val editor = fileEditor.editor

            val uri = file.url

            val rev = revisions[uri]!!
            rev.editor += 1u

            // TODO: this calc doesn't seem right because there are some odd changes on the Neovim instance
            val start = editor.offsetToLogicalPosition(event.newRange.startOffset)
            val end = editor.offsetToLogicalPosition(event.newRange.endOffset)

            launchEditRequest(
               EditRequest(
                  uri,
                  rev.daemon,
                  Collections.singletonList(Delta(
                     Range(
                        Position(start.line, start.column),
                        Position(end.line, end.column)
                     ),
                     // TODO: I remember UTF-16/32… did not test a none ASCII file yet
                     event.newFragment.toString()
                  ))
               )
            )
         }
      }

      for (editor in FileEditorManager.getInstance(project).allEditors) {
         if (editor is TextEditor) {
            editor.editor.caretModel.addCaretListener(caretListener)
            editor.editor.document.addDocumentListener(documentListener)
         }
      }

      EditorFactory.getInstance().addEditorFactoryListener(object : EditorFactoryListener {
         override fun editorCreated(event: EditorFactoryEvent) {
            event.editor.caretModel.addCaretListener(caretListener)
            event.editor.document.addDocumentListener(documentListener)
         }

         override fun editorReleased(event: EditorFactoryEvent) {
            event.editor.caretModel.removeCaretListener(caretListener)
            event.editor.document.removeDocumentListener(documentListener)
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
         it.destroy()
         it.awaitExit()
         daemonProcess = null
      }
      revisions.clear()
   }

   override fun connectToPeer(peer: String) {
      val projectDirectory = File(project.basePath!!)
      val ethersyncDirectory = File(projectDirectory, ".ethersync")
      val socket = "ethersync-%s-socket".format(project.name)

      cs.launch {
         if (!ethersyncDirectory.exists()) {
            LOG.debug("Creating ethersync directory")
            ethersyncDirectory.mkdir()
         }

         val notifier = project.messageBus.syncPublisher(DaemonOutputNotifier.CHANGE_ACTION_TOPIC)
         if (daemonProcess != null || clientProcess != null) {
            notifier.clear()
            shutdown()
         }

         LOG.info("Starting ethersync daemon")

         // TODO: try catch not existing binary
         val daemonProcessBuilder = ProcessBuilder(
            AppSettings.getInstance().state.ethersyncBinaryPath,
            "daemon",
            "--peer", peer,
            "--socket-name", socket)
            .directory(projectDirectory)
         daemonProcess = daemonProcessBuilder.start()
         val daemonProcess = daemonProcess!!

         cs.launch {
            val stdout = BufferedReader(InputStreamReader(daemonProcess.inputStream))
            stdout.use {
               while (true) {
                  val line = try {
                      stdout.readLineAsync() ?: break;
                  } catch (e: IOException) {
                     LOG.error(e)
                     break
                  }
                  LOG.trace(line)
                  cs.launch {
                     withContext(Dispatchers.EDT) {
                        notifier.logOutput(line)
                     }
                  }

                  if (line.contains("Others can connect with")) {
                     launchEthersyncClient(socket, projectDirectory)
                  }
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
                  cs.launch {
                     withContext(Dispatchers.EDT) {
                        notifier.logOutput(line)
                     }
                  }
               }
            }

            withContext(Dispatchers.EDT) {
               notifier.logOutput("ethersync exited with exit code: " + daemonProcess.exitValue())
            }
         }
      }
   }

   private fun createProtocolHandler(): EthersyncEditorProtocol {
      val highlighter = HashMap<String, List<RangeHighlighter>>()

      return object : EthersyncEditorProtocol {
         override fun cursor(cursorEvent: CursorEvent) {
            val fileEditorManager = FileEditorManager.getInstance(project)

            val fileEditor = fileEditorManager.allEditors
               .firstOrNull { editor -> editor.file.url == cursorEvent.documentUri } ?: return

            if (fileEditor is TextEditor) {
               val editor = fileEditor.editor

               cs.launch {
                  withContext(Dispatchers.EDT) {
                     synchronized(highlighter) {
                        val markupModel = editor.markupModel

                        val previous = highlighter.remove(cursorEvent.userId)
                        if (previous != null) {
                           for (hl in previous) {
                              markupModel.removeHighlighter(hl)
                           }
                        }

                        val newHighlighter = LinkedList<RangeHighlighter>()
                        for(range in cursorEvent.ranges) {
                           val startPosition = editor.logicalPositionToOffset(LogicalPosition(range.start.line, range.start.character))
                           val endPosition = editor.logicalPositionToOffset(LogicalPosition(range.end.line, range.end.character))

                           val textAttributes = TextAttributes().apply {
                              // foregroundColor = JBColor(JBColor.YELLOW, JBColor.DARK_GRAY)

                              // TODO: unclear which is the best effect type
                              effectType = EffectType.ROUNDED_BOX
                              effectColor = JBColor(JBColor.YELLOW, JBColor.DARK_GRAY)
                           }

                           val hl = markupModel.addRangeHighlighter(
                              startPosition,
                              endPosition + 1,
                              HighlighterLayer.ADDITIONAL_SYNTAX,
                              textAttributes,
                              HighlighterTargetArea.EXACT_RANGE
                           )
                           if (cursorEvent.name != null) {
                              hl.errorStripeTooltip = cursorEvent.name
                           }

                           newHighlighter.add(hl)
                        }
                        highlighter[cursorEvent.userId] = newHighlighter
                     }
                  }
               }
            }
         }

         override fun edit(editEvent: EditEvent) {
            val revision = revisions[editEvent.documentUri]!!

            // Check if operation is up-to-date to our content.
            // If it's not, ignore it! The daemon will send a transformed one later.
            if (editEvent.editorRevision == revision.editor) {
               ignoreChangeEvent.set(true)

               val fileEditorManager = FileEditorManager.getInstance(project)

               val fileEditor = fileEditorManager.allEditors
                  .first { editor -> editor.file.url == editEvent.documentUri } ?: return

               if (fileEditor is TextEditor) {
                  val editor = fileEditor.editor

                  WriteCommandAction.runWriteCommandAction(project, {
                     for(delta in editEvent.delta) {
                        val start = editor.logicalPositionToOffset(LogicalPosition(delta.range.start.line, delta.range.start.character))
                        val end = editor.logicalPositionToOffset(LogicalPosition(delta.range.end.line, delta.range.end.character))

                        editor.document.replaceString(start, end, delta.replacement)
                     }
                  })

                  revision.daemon += 1u

                  ignoreChangeEvent.set(false)
               }
            }
         }

      }
   }

   private fun launchEthersyncClient(socket: String, projectDirectory: File) {
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

         val fileEditorManager = FileEditorManager.getInstance(project)
         for (file in fileEditorManager.openFiles) {
            launchDocumentOpenRequest(file.url)
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
                  System.out.println(line)
               }
            }
         }
      }
   }

   fun launchDocumentCloseNotification(fileUri: String) {
      val launcher = launcher ?: return
      cs.launch {
         launcher.remoteProxy.close(DocumentRequest(fileUri))
         revisions.remove(fileUri)
      }
   }

   fun launchDocumentOpenRequest(fileUri: String) {
      val launcher = launcher ?: return
      cs.launch {
         try {
            revisions[fileUri] = FileRevision();
            launcher.remoteProxy.open(DocumentRequest(fileUri)).await()
         } catch (e: ResponseErrorException) {
            TODO("not yet implemented: notify about an protocol error")
         }
      }
   }

   fun launchCursorRequest(cursorRequest: CursorRequest) {
      val launcher = launcher ?: return
      cs.launch {
         try {
            launcher.remoteProxy.cursor(cursorRequest).await()
         } catch (e: ResponseErrorException) {
            TODO("not yet implemented: notify about an protocol error")
         }
      }
   }

   fun launchEditRequest(editRequest: EditRequest) {
      val launcher = launcher ?: return
      cs.launch {
         try {
            launcher.remoteProxy.edit(editRequest).await()
         } catch (e: ResponseErrorException) {
            TODO("not yet implemented: notify about an protocol error")
         }
      }
   }
}
