package io.github.ethersync.protocol

import com.google.gson.annotations.SerializedName

data class EditRequest(
   @SerializedName("uri")
   val documentUri: String,
   @SerializedName("revision")
   val revision: UInt,
   @SerializedName("delta")
   val delta: List<Delta>,
)