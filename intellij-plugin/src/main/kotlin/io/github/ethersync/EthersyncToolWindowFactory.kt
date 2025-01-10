package io.github.ethersync

import com.intellij.execution.filters.TextConsoleBuilderFactory
import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory

class EthersyncToolWindowFactory : ToolWindowFactory {

    override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
       val console = TextConsoleBuilderFactory.getInstance()
            .createBuilder(project)
            .console

        val content = ContentFactory.getInstance()
            .createContent(console.component, "Daemon", true)

        toolWindow.contentManager.addContent(content)
    }
}