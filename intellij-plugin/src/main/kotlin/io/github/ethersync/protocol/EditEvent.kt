package io.github.ethersync.protocol

import com.google.gson.annotations.SerializedName

data class EditEvent(
   @SerializedName("uri")
   val documentUri: String,
   @SerializedName("revision")
   val editorRevision: UInt,
   @SerializedName("delta")
   val delta: List<Delta>,
)