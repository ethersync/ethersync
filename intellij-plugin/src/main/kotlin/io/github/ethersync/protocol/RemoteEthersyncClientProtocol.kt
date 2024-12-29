package io.github.ethersync.protocol

import com.google.gson.JsonObject
import org.eclipse.lsp4j.jsonrpc.services.JsonNotification
import org.eclipse.lsp4j.jsonrpc.services.JsonRequest
import java.util.concurrent.CompletableFuture

interface RemoteEthersyncClientProtocol {
    @JsonRequest
    fun cursor(cursorRequest: CursorRequest): CompletableFuture<JsonObject>

    @JsonRequest
    fun open(documentRequest: DocumentRequest): CompletableFuture<JsonObject>

    @JsonNotification
    fun close(documentRequest: DocumentRequest)
}