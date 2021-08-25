<script>
    import * as Y from "yjs"

    import {onMount, createEventDispatcher} from "svelte"
    import {shortcut} from "./shortcut.js"

    import {CodemirrorBinding} from "y-codemirror"
    import CodeMirror from "codemirror"

    const dispatch = createEventDispatcher()

    let editorDiv, editor, yUndoManager, binding
    export let ytext, awareness, pages

    function linkOverlay(pages) {
        const query = new RegExp(pages.join("|"), "gi")

        return {
            token: function (stream) {
                query.lastIndex = stream.pos
                var match = query.exec(stream.string)
                if (match && match.index == stream.pos) {
                    stream.pos += match[0].length || 1
                    return "link"
                } else if (match) {
                    stream.pos = match.index
                } else {
                    stream.skipToEnd()
                }
            },
            name: "links",
        }
    }

    function urlOverlay() {
        const query = /\b(https?:\/\/\S*\b)/g

        return {
            token: function (stream) {
                query.lastIndex = stream.pos
                var match = query.exec(stream.string)
                if (match && match.index == stream.pos) {
                    stream.pos += match[0].length || 1
                    return "url"
                } else if (match) {
                    stream.pos = match.index
                } else {
                    stream.skipToEnd()
                }
            },
        }
    }

    $: if (editorDiv) {
        /*if (editor) {
            editor.getWrapperElement().remove()
        }*/
        if (!editor) {
            editor = CodeMirror(editorDiv, {
                lineNumbers: true,
                flattenSpans: false,
                lineWrapping: true,
            })

            editor.addOverlay(urlOverlay())

            editor.getWrapperElement().addEventListener("mousedown", (e) => {
                if (e.which == 1) {
                    if (e.target.classList.contains("cm-link")) {
                        let title = e.target.innerHTML
                        dispatch("openPage", {title})
                    }
                    if (e.target.classList.contains("cm-url")) {
                        let url = e.target.innerHTML
                        window.open(url, "_blank")
                    }
                }
            })
        }

        if (binding && binding.doc === ytext) {
            // No need to do anything.
        } else {
            if (binding) {
                console.log("destroy binding")
                binding.destroy()
            }
            yUndoManager = new Y.UndoManager(ytext)
            binding = new CodemirrorBinding(ytext, editor, awareness, {
                yUndoManager,
            })
        }
    }

    $: if (editor && pages) {
        console.log("updating overlay")
        editor.removeOverlay("links")
        editor.addOverlay(linkOverlay(pages))
    }

    function currentDate() {
        var today = new Date()
        return (
            today.getFullYear().toString().padStart(2, "0") +
            "-" +
            (today.getMonth() + 1).toString().padStart(2, "0") +
            "-" +
            today.getDate().toString().padStart(2, "0")
        )
    }

    function applyGoogleKeyboardWorkaround(editor) {
        try {
            if (editor.applyGoogleKeyboardWorkaround) {
                return
            }

            editor.applyGoogleKeyboardWorkaround = true
            editor.on("editor-change", function (eventName, ...args) {
                if (eventName === "text-change") {
                    // args[0] will be delta
                    var ops = args[0]["ops"]
                    var oldSelection = editor.getSelection()
                    var oldPos = oldSelection.index
                    var oldSelectionLength = oldSelection.length

                    if (
                        ops[0]["retain"] === undefined ||
                        !ops[1] ||
                        !ops[1]["insert"] ||
                        !ops[1]["insert"] ||
                        ops[1]["insert"] != "\n" ||
                        oldSelectionLength > 0
                    ) {
                        return
                    }

                    setTimeout(function () {
                        var newPos = editor.getSelection().index
                        if (newPos === oldPos) {
                            console.log("Change selection bad pos")
                            editor.setSelection(
                                editor.getSelection().index + 1,
                                0,
                            )
                        }
                    }, 30)
                }
            })
        } catch {}
    }
</script>

<div
    class="editor flex-grow"
    bind:this={editorDiv}
    use:shortcut={{
        code: "End",
        callback: () => {
            quill.setSelection(quill.getLength(), 0)
        },
    }}
    use:shortcut={{
        code: "F9",
        callback: () => {
            if (quill.hasFocus()) {
                quill.insertText(quill.getSelection(), currentDate())
            }
        },
    }}
/>

<svelte:head>
    <link
        href="https://pvinis.github.io/iosevka-webfont/3.4.1/iosevka.css"
        rel="stylesheet"
    />
    <link
        rel="stylesheet"
        href="https://codemirror.net/lib/codemirror.css"
        async
        defer
    />
</svelte:head>

<style>
    .editor {
        font-family: "Iosevka Web" !important;
        font-size: 105%;
    }
    :global(.CodeMirror) {
        height: 100% !important;
        width: 100% !important;
    }
    :global(.cm-link),
    :global(.cm-url) {
        cursor: pointer;
        font-weight: bold;
        color: darkblue !important;
        text-decoration: none !important;
    }
    :global(.remote-caret) {
        position: absolute;
        border-left: black;
        border-left-style: solid;
        border-left-width: 2px;
        height: 1em;
    }
    :global(.remote-caret > div) {
        position: relative;
        top: -1.05em;
        font-size: 13px;
        background-color: rgb(250, 129, 0);
        font-family: serif;
        font-style: normal;
        font-weight: normal;
        line-height: normal;
        user-select: none;
        color: white;
        padding-left: 2px;
        padding-right: 2px;
        z-index: 3;
    }
</style>
