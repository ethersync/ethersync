package io.github.ethersync

import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory
import org.fusesource.jansi.utils.UtilsAnsiHtml
import org.jsoup.Jsoup
import javax.swing.JEditorPane
import javax.swing.JScrollPane

class ToolWindowFactory : ToolWindowFactory {

   override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
      val logTextArea = JEditorPane()
      logTextArea.contentType = "text/html"
      logTextArea.isEditable = false

      val utilsAnsiHtml = UtilsAnsiHtml()
      // TODO: the font-family doesn't have an effect
      val document = Jsoup.parse("""
         <html>
            <head>
               <style>
                  body {
                     font-family: 'Courier New', Courier, monospace;
                  }
               </style>
            </head>
            <body></body>
         </html>
         """.trimIndent())
      val body = document.getElementsByTag("body")

      project.messageBus.connect().subscribe(
         DaemonOutputNotifier.CHANGE_ACTION_TOPIC,
         object : DaemonOutputNotifier {
            override fun logOutput(line: String) {
               body.append(utilsAnsiHtml.convertAnsiToHtml(line))
               body.append("<br>")
               logTextArea.text = document.html()
            }
         }
      )

      val content = ContentFactory.getInstance().createContent(JScrollPane(logTextArea), "Daemon Log", false)
      toolWindow.contentManager.addContent(content)
   }
}