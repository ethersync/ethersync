package io.github.ethersync.protocol

import com.intellij.openapi.components.Service
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.project.Project

@Service(Service.Level.PROJECT)
class EthersyncEditorProtocolImpl(private val project: Project) : EthersyncEditorProtocol {
   override fun cursor(cursorEvent: CursorEvent) {
      System.out.printf("Cursor: %s, %s\n", cursorEvent.documentUri, cursorEvent.ranges)

      val fileEditorManager = FileEditorManager.getInstance(project);
      val editor = fileEditorManager.allEditors
         .find { editor -> editor.file.url == cursorEvent.documentUri } ?: return

      // TODO find a way how to create an additional cursor
   }
}