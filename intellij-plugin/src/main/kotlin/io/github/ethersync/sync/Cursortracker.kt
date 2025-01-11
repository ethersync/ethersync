package io.github.ethersync.sync

import com.intellij.openapi.application.EDT
import com.intellij.openapi.editor.LogicalPosition
import com.intellij.openapi.editor.event.CaretEvent
import com.intellij.openapi.editor.event.CaretListener
import com.intellij.openapi.editor.markup.*
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.TextEditor
import com.intellij.openapi.project.Project
import com.intellij.ui.JBColor
import com.intellij.util.io.await
import io.github.ethersync.protocol.CursorEvent
import io.github.ethersync.protocol.CursorRequest
import io.github.ethersync.protocol.RemoteEthersyncClientProtocol
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.eclipse.lsp4j.Position
import org.eclipse.lsp4j.Range
import org.eclipse.lsp4j.jsonrpc.ResponseErrorException
import java.util.*

class Cursortracker(
   private val project: Project,
   private val cs: CoroutineScope,
) : CaretListener {

   private val highlighter = HashMap<String, List<RangeHighlighter>>()

   var remoteProxy: RemoteEthersyncClientProtocol? = null

   fun handleRemoteCursorEvent(cursorEvent: CursorEvent) {
      val fileEditor = FileEditorManager.getInstance(project)
         .allEditors
         .filterIsInstance<TextEditor>()
         .filter { editor -> editor.file.canonicalFile != null }
         .firstOrNull { editor -> editor.file.canonicalFile!!.url == cursorEvent.documentUri } ?: return

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

   override fun caretPositionChanged(event: CaretEvent) {
      val canonicalFile = event.editor.virtualFile?.canonicalFile ?: return
      val uri = canonicalFile.url
      val pos = Position(event.newPosition.line, event.newPosition.column)
      val range = Range(pos, pos)
      launchCursorRequest(CursorRequest(uri, Collections.singletonList(range)))
   }

   private fun launchCursorRequest(cursorRequest: CursorRequest) {
      val remoteProxy = remoteProxy ?: return
      cs.launch {
         try {
            remoteProxy.cursor(cursorRequest).await()
         } catch (e: ResponseErrorException) {
            TODO("not yet implemented: notify about an protocol error")
         }
      }
   }

   fun clear() {
      remoteProxy = null
   }
}