<script>
    import { onMount } from 'svelte';

    import * as Y from 'yjs'
    import { WebrtcProvider } from 'y-webrtc'

    import { QuillBinding } from 'y-quill'
    import Quill from 'quill'
    import QuillCursors from 'quill-cursors'

    Quill.register('modules/cursors', QuillCursors)

    onMount(() => {
        const ydoc = new Y.Doc()
        const ytext = ydoc.getText("content")

        const provider = new WebrtcProvider(`svelte-yjs-experiment`, ydoc, {
            signaling: ['wss://signaling.yjs.dev', 'wss://y-webrtc-signaling-eu.herokuapp.com', 'wss://y-webrtc-signaling-us.herokuapp.com']
        })

        console.log(document.querySelector("#content"))
        const quill = new Quill(document.querySelector("#content"), {
            modules: {
                cursors: true,
                toolbar: false,
                history: {
                    userOnly: true
                }
            },
            formats: [],
        })
        quill.root.setAttribute('spellcheck', false)
        const binding = new QuillBinding(ytext, quill)
    })

    let title = "foobar"
    let pages = [
        {title: "Animals", content: "- Cats\n- Dogs"},
        {title: "Fruit", content: "- Apple\n- Pear"},
    ]
</script>

<svelte:head>
    <link
        href="https://unpkg.com/tailwindcss@^2.0/dist/tailwind.min.css"
        rel="stylesheet"
    />
    <link rel="stylesheet" type="text/css" href="https://cdn.quilljs.com/1.3.6/quill.snow.css">
</svelte:head>

<div class="flex-col h-screen">
    <div class="flex bg-gray-200">
        <div id="room" class="p-2 font-bold w-60">{title}</div>
        <!--<input id="search" placeholder="Search..." class="m-2 px-3 py-1 w-60">-->
        <div class="flex-1"></div>
        <div id="export" class="p-2 cursor-pointer hover:bg-gray-500 text-center">📥 Export zip</div>
        <div style="display: grid;" class="hover:bg-gray-500 hover:cursor-pointer w-40">
            <input type="file" id="import" multiple style="grid-column: 1; grid-row: 1;" class="cursor-pointer">
            <span style="grid-column: 1; grid-row: 1;" class="p-2 text-center">📤 Upload files</span>
        </div>
        <div id="delete-all" class="p-2 cursor-pointer hover:bg-gray-500 text-center">💣 Delete all</div>
        <div class="dropdown relative">
            <div class="bg-gray-300 text-gray-700 font-semibold py-2 px-4 flex place-items-end items-center w-60">
              <span class="mr-1" id="connection-status">unknown</span>
              <svg class="fill-current h-4 w-4" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path d="M9.293 12.95l.707.707L15.657 8l-1.414-1.414L10 10.828 5.757 6.586 4.343 8z"/> </svg>
            </div>
            <ul class="dropdown-menu absolute hidden text-gray-700 pt-1 bg-gray-100 w-60">
              <li class="">
                    <input id="username" type="text" class="m-2 p-1 font-mono" autocomplete="off"/>
              </li>
              <div id="users">
              </div>
            </ul>
          </div>
    </div>
    <div class="flex flex-col">
        <div class="flex flex-1">
            <div class="flex flex-col bg-gray-300 w-60 h-screen">
                <div class="flex flex-col overflow-y-auto" id="docs">
                    {#each pages as {title}, i}
                        <div class="doc-button p-2 border-b border-gray-400 flex hover:bg-gray-400 cursor-pointer" data-id={i}>{title}</div>
                    {/each}
                </div>
                <div
                    id="add-button"
                    class="p-2 hover:bg-blue-400 text-center cursor-pointer"
                >
                    ➕ Add page
                </div>
                <div class="flex-1"></div>
            </div>
            <div class="w-full flex flex-col">
                <div id="title"></div>
                <div id="content" class="flex-grow"></div>
            </div>
        </div>
    </div>
</div>

<style>
    #title {
        border-bottom: 1px solid lightgray;
    }
    #room, #title {
        height: 2.8em;
        font-size: 1.1rem;
    }
    #content {
        font-family: monospace;
        height: 10em;
    }
    input[type=file] {
        opacity: 0.01;
    }
    .dropdown:hover .dropdown-menu {
        display: block;
    }
</style>
