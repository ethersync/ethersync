<script>
    import {onMount} from "svelte"
    import {shortcut} from "./shortcut.js"

    import {QuillBinding} from "y-quill"
    import Quill from "quill"
    import QuillCursors from "quill-cursors"

    Quill.register("modules/cursors", QuillCursors)

    let editor
    export let ytext, awareness

    let quill
    $: if (editor) {
        quill = new Quill(editor, {
            modules: {
                cursors: true,
                toolbar: false,
                history: {
                    userOnly: true,
                },
            },
            formats: [],
        })
        quill.root.setAttribute("spellcheck", false)
        const binding = new QuillBinding(ytext, quill, awareness)

        // See https://github.com/quilljs/quill/issues/3240
        applyGoogleKeyboardWorkaround(quill)

        if (quill.getText() == "New Page\n") {
            selectAll()
        }
    }

    function selectAll() {
        quill.setSelection(0, quill.getLength())
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
    bind:this={editor}
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
        rel="stylesheet"
        type="text/css"
        href="https://cdn.quilljs.com/1.3.6/quill.snow.css"
    />
    <link
        href="https://pvinis.github.io/iosevka-webfont/3.4.1/iosevka.css"
        rel="stylesheet"
    />
</svelte:head>

<style>
    .editor {
        font-family: "Iosevka Web" !important;
        font-size: 105%;
    }
</style>
