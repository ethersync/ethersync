package io.github.ethersync.protocol

import com.google.gson.annotations.SerializedName
import org.eclipse.lsp4j.Range

data class CursorRequest(
   @SerializedName("uri")
   val uri: String,
   @SerializedName("ranges")
   val ranges: List<Range>
)
