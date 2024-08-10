;;; ethersync.el --- ??? -*- lexical-binding: t -*-

;; Author: Danny McClanahan
;; Version: 0.0
;; URL: https://github.com/ethersync/ethersync
;; Package-Requires: ((emacs "25.2") (cl-lib "0.5") (dash "2"))
;; Keywords: ???

;; This file is not part of GNU Emacs.

;; This file is free software: you can redistribute it and/or modify it under the terms of the
;; GNU Affero General Public License as published by the Free Software Foundation, either version 3
;; of the License, or (at your option) any later version.

;; This file is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
;; GNU General Public License for more details.

;; You should have received a copy of the GNU Affero General Public License
;; along with this program. If not, see <http://www.gnu.org/licenses/>.


;;; Commentary:

;; ???


;; Usage:

;; ???


;; License:

;; AGPL 3.0+

;; End Commentary


;;; Code:

(require 'cl-lib)
(require 'dash)
(require 'jsonrpc)
(require 'pcase)
(require 'rx)
(require 'subr-rx)


;; Customization Helpers
(defun ethersync--always-safe-local (_)
  "Use as a :safe predicate in a `defcustom' form to accept any local override."
  t)


;; Public error types
(define-error 'ethersync-error "Error using ethersync.")
(define-error 'ethersync-client-connect-error "Error establishing a client connection"
              'ethersync-error)
(define-error 'ethersync-rpc-error "Error processing a json-rpc message."
              '(jsonrpc-error ethersync-error))


;; Customization
(defgroup ethersync nil
  "Group for `ethersync' customizations."
  :group 'files
  :group 'shadow)

(defcustom ethersync-socket-location "/tmp/ethersync"
  "File path to the unix domain socket used by ethersync."
  :type 'file
  :safe #'ethersync--always-safe-local
  :group 'ethersync)


;; Constants
(defconst ethersync--process-name
  "ethersync"
  "Name of the network process to spawn with `make-network-process'.

This name will be modified by `make-network-process' to make it unique, if multiple ethersync
clients are spawned at once.")

(defconst ethersync--connection-name
  "ethersync-emacs-client"
  "NAME argument for the `jsonrpc-connection' instance to spawn.")

(defconst ethersync--process-buffer-name-base
  " *ethersync-buf*"
  "Base name of the buffer used for the network process.

This name is used as the basis to generate a new buffer name for an ethersync client process.

An initial space is used to hide the buffer from the buffer list.")


;; Variables
(defvar ethersync--active-clients nil
  "The list of client processes currently active in the background.")


;; Buffer-local Variables
(defvar-local ethersync-controlling-client nil
  "The ethersync client process making edits to this buffer, if applicable.")


;; Class definitions
(defclass ethersync-client (jsonrpc-connection)
  ((-process
    :initarg :process :type process :accessor ethersync--client-process
    :documentation "Process object wrapped by this client."))
  :documentation "An ethersync JSONRPC connection over a socket.
The following initargs are accepted:

:PROCESS (mandatory), a live network process object to a unix domain socket. The socket will be
reading and writing JSONRPC messages with basic HTTP-style enveloping headers such as
\"Content-Length:\".")

(cl-defmethod initialize-instance :after ((client ethersync-client)
                                          &key process &allow-other-keys)
  (process-put process 'jsonrpc-connection client))

(cl-defmethod ethersync--client-socket-path ((client ethersync-client))
  "Retrieve the path to the unix domain socket being used by CLIENT."
  (ethersync--get-process-socket-path
   (ethersync--client-process client)))

(cl-defmethod jsonrpc-connection-send ((client ethersync-client)
                                       &key method params)
  "Send a JSONRPC message with METHOD and PARAMS to connection CLIENT."
  (cl-check-type method symbol "ethersync method name must be a symbol")
  (let* ((args `(:method ,(symbol-name method) :params ,params))
         (converted (jsonrpc-convert-to-endpoint client args 'notification))
         (json (jsonrpc--json-encode converted))
         (headers `(("Content-Length" . ,(format "%d" (string-bytes json))))))
    (let ((proc (jsonrpc--process client))
          (complete-encoded-message
           (cl-loop for (header . value) in headers
                    concat (concat header ": " value "\r\n") into header-section
                    finally return (format "%s\r\n%s" header-section json))))
      (process-send-string proc complete-encoded-message)
      (jsonrpc--event
       client 'client
       :json json
       :kind 'notification
       :message args
       :foreign-message converted))))


;; Utilities
(defun ethersync--get-file-mode (path)
  (-> (file-attributes path)
      (file-attribute-modes)
      (aref 0)))

(defun ethersync--file-is-socket (path)
  (let ((mode (ethersync--get-file-mode path)))
    (char-equal mode ?s)))

(defun ethersync--assert-socket-path (socket-location)
  (unless (file-exists-p socket-location)
    (signal 'ethersync-client-connect-error
            `("[ethersync] socket path does not exist" ,socket-location)))
  (unless (ethersync--file-is-socket path)
    (signal 'ethersync-client-connect-error
            `("[ethersync] path is not a socket" ,socket-location))))

(defun ethersync--get-process-socket-path (proc)
  "Get the unix domain socket path used by PROC."
  (cl-destructuring-bind (&key service &allow-other-keys)
      (process-contact proc t)
    (ethersync--assert-socket-path service)
    service))


;; Logic
(defun ethersync--create-client-process (socket-location)
  (ethersync--assert-socket-path socket-location)
  (let* ((new-buffer-name (generate-new-buffer ethersync--process-buffer-name t))
         (socket-proc (make-network-process
                       :name ethersync--process-name
                       :buffer new-buffer-name
                       :service socket-location
                       :family 'local
                       :coding 'no-conversion
                       :noquery t
                       :filter #'ethersync--process-filter
                       :sentinel #'ethersync--process-sentinel)))
    (make-instance
     'ethersync-client
     :name ethersync--connection-name
     :process socket-proc)))

(defun ethersync--process-filter (proc string))

(defun ethersync--process-sentinel (proc change))


;; Autoloaded functions

(provide 'ethersync)
;;; ethersync.el ends here
