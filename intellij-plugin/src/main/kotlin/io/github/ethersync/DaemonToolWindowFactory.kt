package io.github.ethersync

import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory
import javax.swing.JScrollPane
import javax.swing.JTextArea

class DaemonToolWindowFactory : ToolWindowFactory {

   override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
      val logTextArea = JTextArea()
      val panel = JScrollPane(logTextArea)

      project.messageBus.connect().subscribe(
         DaemonOutputNotifier.CHANGE_ACTION_TOPIC,
         object : DaemonOutputNotifier {
            override fun logOutput(line: String) {
               logTextArea.append(line)
               logTextArea.append("\n")
            }
         }
      )

      val content = ContentFactory.getInstance().createContent(panel, "Ethersync Daemon", false)
      toolWindow.contentManager.addContent(content)
   }
}