package io.github.ethersync.sync

import com.intellij.openapi.command.WriteCommandAction
import com.intellij.openapi.editor.LogicalPosition
import com.intellij.openapi.editor.event.DocumentEvent
import com.intellij.openapi.editor.event.DocumentListener
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.TextEditor
import com.intellij.openapi.project.Project
import com.intellij.refactoring.suggested.oldRange
import com.intellij.util.io.await
import io.github.ethersync.protocol.Delta
import io.github.ethersync.protocol.EditEvent
import io.github.ethersync.protocol.EditRequest
import io.github.ethersync.protocol.RemoteEthersyncClientProtocol
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.eclipse.lsp4j.Position
import org.eclipse.lsp4j.Range
import org.eclipse.lsp4j.jsonrpc.ResponseErrorException
import java.util.*
import java.util.concurrent.atomic.AtomicBoolean

class Changetracker(
   private val project: Project,
   private val cs: CoroutineScope,
) : DocumentListener {

   private val ignoreChangeEvent = AtomicBoolean(false)

   private data class FileRevision(
      // Number of operations the daemon has made.
      var daemon: UInt = 0u,
      // Number of operations we have made.
      var editor: UInt = 0u,
   )
   private val revisions: HashMap<String, FileRevision> = HashMap()

   var remoteProxy: RemoteEthersyncClientProtocol? = null

   override fun documentChanged(event: DocumentEvent) {
      if (ignoreChangeEvent.get()) {
         return
      }

      val file = FileDocumentManager.getInstance().getFile(event.document)!!
      val fileEditor = FileEditorManager.getInstance(project).getEditors(file)
         .filterIsInstance<TextEditor>()
         .first()

      val editor = fileEditor.editor

      val uri = file.canonicalFile!!.url

      val rev = revisions[uri]!!
      rev.editor += 1u

      val start = editor.offsetToLogicalPosition(event.oldRange.startOffset)
      val end = editor.offsetToLogicalPosition(event.oldRange.endOffset)

      launchEditRequest(
         EditRequest(
            uri,
            rev.daemon,
            Collections.singletonList(
               Delta(
               Range(
                  Position(start.line, start.column),
                  Position(end.line, end.column)
               ),
               // TODO: I remember UTF-16/32… did not test a none ASCII file yet
               event.newFragment.toString()
            )
            )
         )
      )
   }

   fun handleRemoteEditEvent(editEvent: EditEvent) {
      val revision = revisions[editEvent.documentUri]!!

      // Check if operation is up-to-date to our content.
      // If it's not, ignore it! The daemon will send a transformed one later.
      if (editEvent.editorRevision == revision.editor) {
         ignoreChangeEvent.set(true)

         val fileEditorManager = FileEditorManager.getInstance(project)

         val fileEditor = fileEditorManager.allEditors
            .first { editor -> editor.file.canonicalFile!!.url == editEvent.documentUri } ?: return

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

   fun openFile(fileUri: String) {
      revisions[fileUri] = FileRevision();
   }

   fun closeFile(fileUri: String) {
      revisions.remove(fileUri)
   }

   fun clear() {
      remoteProxy = null
      revisions.clear()
   }

   private fun launchEditRequest(editRequest: EditRequest) {
      val remoteProxy = remoteProxy ?: return
      cs.launch {
         try {
            remoteProxy.edit(editRequest).await()
         } catch (e: ResponseErrorException) {
            TODO("not yet implemented: notify about an protocol error")
         }
      }
   }
}