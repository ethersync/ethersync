package io.github.ethersync.protocol

import com.google.gson.annotations.SerializedName
import org.eclipse.lsp4j.Range

data class Delta(
   @SerializedName("range")
   val range: Range,
   @SerializedName("replacement")
   val replacement: String
)
