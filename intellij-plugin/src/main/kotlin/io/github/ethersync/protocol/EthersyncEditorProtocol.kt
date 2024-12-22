package io.github.ethersync.protocol

import org.eclipse.lsp4j.jsonrpc.services.JsonNotification

interface EthersyncEditorProtocol {
   @JsonNotification
   fun cursor(cursorEvent: CursorEvent)
}