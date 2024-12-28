package io.github.ethersync.protocol

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.application.ModalityState
import com.intellij.openapi.components.Service
import com.intellij.openapi.editor.LogicalPosition
import com.intellij.openapi.editor.markup.HighlighterLayer
import com.intellij.openapi.editor.markup.HighlighterTargetArea
import com.intellij.openapi.editor.markup.RangeHighlighter
import com.intellij.openapi.editor.markup.TextAttributes
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.TextEditor
import com.intellij.openapi.project.Project
import com.intellij.ui.JBColor
import java.util.LinkedList

@Service(Service.Level.PROJECT)
class EthersyncEditorProtocolImpl(private val project: Project) : EthersyncEditorProtocol {

   private val highlighter = LinkedList<RangeHighlighter>()

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
}
