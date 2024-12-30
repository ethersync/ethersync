package io.github.ethersync

import com.intellij.util.messages.Topic

interface DaemonOutputNotifier {
   companion object {

      @JvmField
      @Topic.ProjectLevel
      val CHANGE_ACTION_TOPIC: Topic<DaemonOutputNotifier> =
         Topic.create("ethersync daemon output", DaemonOutputNotifier::class.java)
   }

   fun clear()

   fun logOutput(line: String)
}