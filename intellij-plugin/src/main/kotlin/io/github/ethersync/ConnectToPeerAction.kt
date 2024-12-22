package io.github.ethersync

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.components.service
import com.intellij.openapi.ui.Messages

class ConnectToPeerAction : AnAction() {

   override fun actionPerformed(e: AnActionEvent) {
      val project = e.project ?: return

      val address = Messages.showInputDialog(project, "Provide ethersync peer address", "Peer address", null)
      if (address != null) {
         val service = project.service<EthersyncService>()

         service.connectToPeer(address)
      }
   }
}
