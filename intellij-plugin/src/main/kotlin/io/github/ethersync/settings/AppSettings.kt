package io.github.ethersync.settings

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import org.jetbrains.annotations.NonNls

@State(
   name = "io.github.ethersync.settings.AppSettings",
   storages = [Storage("EthersyncSettingsPlugin.xml")]
)
class AppSettings : PersistentStateComponent<AppSettings.State> {

   data class State(
      @NonNls
      var ethersyncBinaryPath: String = "ethersync"
   )

   private var state: State = State()

   companion object {
      fun getInstance() : AppSettings {
         return ApplicationManager.getApplication().getService(AppSettings::class.java)
      }
   }

   override fun getState(): State {
      return state
   }

   override fun loadState(state: State) {
      this.state = state
   }
}