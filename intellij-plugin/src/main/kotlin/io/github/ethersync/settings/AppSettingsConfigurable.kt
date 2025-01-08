package io.github.ethersync.settings

import com.intellij.openapi.options.Configurable
import org.jetbrains.annotations.Nls
import javax.swing.JComponent

class AppSettingsConfigurable : Configurable {

   private var compoment: AppSettingsComponent? = null

   override fun createComponent(): JComponent {
      compoment = AppSettingsComponent()
      return compoment!!.panel
   }

   override fun isModified(): Boolean {
      val state = AppSettings.getInstance().state
      return state.ethersyncBinaryPath != compoment!!.ethersyncBinary
   }

   override fun apply() {
      val state = AppSettings.getInstance().state
      state.ethersyncBinaryPath = compoment!!.ethersyncBinary
   }

   @Nls(capitalization = Nls.Capitalization.Title)
   override fun getDisplayName(): String {
      return "Ethersync"
   }

   override fun reset() {
      val state = AppSettings.getInstance().state
      compoment!!.ethersyncBinary = state.ethersyncBinaryPath
   }

   override fun disposeUIResources() {
      compoment = null
   }
}