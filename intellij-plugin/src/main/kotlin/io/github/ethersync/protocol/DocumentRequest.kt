package io.github.ethersync.protocol

import com.google.gson.annotations.SerializedName

data class DocumentRequest(
   @SerializedName("uri")
   val documentUri: String
)