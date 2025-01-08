package io.github.ethersync.settings

import com.intellij.ui.components.JBLabel
import com.intellij.ui.components.JBTextField
import com.intellij.util.ui.FormBuilder
import javax.swing.JPanel

class AppSettingsComponent {

   val panel: JPanel
   private val ethersyncBinaryTF: JBTextField = JBTextField()

   init {
       panel = FormBuilder.createFormBuilder()
          .addLabeledComponent(JBLabel("Ethersync binary:"), ethersyncBinaryTF, 1, false)
          .addComponentFillVertically(JPanel(), 0)
          .panel
   }

   var ethersyncBinary: String
      get() {
         return if (ethersyncBinaryTF.text.isNullOrBlank()) {
            "ethersync"
         } else {
            ethersyncBinaryTF.text.toString()
         }
      }
      set(value) {
         ethersyncBinaryTF.setText(value)
      }
}