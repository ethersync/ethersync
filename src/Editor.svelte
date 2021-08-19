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

        if (quill.getText() == "New Page\n") {
            selectAll()
        }
    }

    function selectAll() {
        quill.setSelection(0, quill.getLength())
    }
</script>

<div
    class="editor"
    bind:this={editor}
    use:shortcut={{
        code: "End",
        callback: () => quill.setSelection(quill.getLength(), 0),
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
