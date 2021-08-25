<script>
    import * as Y from "yjs"

    import {onMount, createEventDispatcher} from "svelte"
    import {shortcut} from "./shortcut.js"

    import {CodemirrorBinding} from "y-codemirror"
    import CodeMirror from "codemirror"

    const dispatch = createEventDispatcher()

    let editorDiv, editor, yUndoManager, binding
    export let ytext,
        awareness,
        pages = []

    function linkOverlay() {
        if (pages === []) {
            return
        }

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
        }
    }

    function urlOverlay() {
        const query = new RegExp(
            "https?://(www.)?[-a-zA-Z0-9@:%._+~#=]{1,256}.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_+.~#?&//=]*)",
        )

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
        if (binding) {
            binding.destroy()
        }

        if (editor) {
            editor.getWrapperElement().remove()
        }

        editor = CodeMirror(editorDiv, {
            lineNumbers: true,
            flattenSpans: false,
        })
        editor.addOverlay(linkOverlay())
        editor.addOverlay(urlOverlay())

        editor.getWrapperElement().addEventListener("mousedown", (e) => {
            if (e.which == 1 && e.target.classList.contains("cm-link")) {
                let title = e.target.innerHTML
                dispatch("openPage", {title})
            }
        })

        yUndoManager = new Y.UndoManager(ytext)

        binding = new CodemirrorBinding(ytext, editor, awareness, {
            yUndoManager,
        })
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
    class="editor"
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
    .CodeMirror {
        height: 100%;
    }
    :global(.cm-link),
    :global(.cm-url) {
        cursor: pointer;
        font-weight: bold;
        color: darkblue !important;
        text-decoration: none !important;
    }
</style>
