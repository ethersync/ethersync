package io.github.ethersync.protocol

import com.google.gson.JsonObject
import org.eclipse.lsp4j.jsonrpc.services.JsonNotification
import org.eclipse.lsp4j.jsonrpc.services.JsonRequest
import java.util.concurrent.CompletableFuture

interface EthersyncEditorProtocol {
   @JsonNotification("cursor")
   fun cursor(cursorEvent: CursorEvent)

}