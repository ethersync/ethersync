<script>
    import {onMount} from "svelte"

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

<div class="editor" bind:this={editor} />

<svelte:head>
    <link
        rel="stylesheet"
        type="text/css"
        href="https://cdn.quilljs.com/1.3.6/quill.snow.css"
    />
</svelte:head>
