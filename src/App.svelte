<script>
    let title = "foobar"

    import Editor from "./Editor.svelte"

    import * as Y from "yjs"
    import {WebrtcProvider} from "y-webrtc"

    const ydoc = new Y.Doc()
    const ypages = ydoc.getArray("pages")

    import {IndexeddbPersistence} from "y-indexeddb"
    const persistence = new IndexeddbPersistence(title, ydoc)

    let pages = ypages.toArray()
    ypages.observeDeep(() => {
        pages = ypages.toArray()
    })

    const provider = new WebrtcProvider(`svelte-yjs-experiment`, ydoc, {
        signaling: [
            "wss://signaling.yjs.dev",
            "wss://y-webrtc-signaling-eu.herokuapp.com",
            "wss://y-webrtc-signaling-us.herokuapp.com",
        ],
    })
    const awareness = provider.awareness

    let awarenessStates = []
    awareness.on("change", () => {
        awarenessStates = [...awareness.getStates()]
    })

    const addPage = () => {
        const ypage = new Y.Map()

        const ytitle = new Y.Text()
        ytitle.insert(0, "New Page")
        ypage.set("title", ytitle)

        const ycontent = new Y.Text()
        ypage.set("content", ycontent)

        ypages.push([ypage])

        currentPage = ypage
    }

    let currentPage = null
    const openPage = (page) => {
        currentPage = page
    }

    const deleteAll = () => {
        if (confirm(`Really delete all pages?`)) {
            ypages.delete(0, ypages.length)
            currentPage = null
        }
    }

    let username = localStorage.getItem("username") || "anonymous"
    $: localStorage.setItem("username", username)

    export const usercolors = [
        "#30bced",
        "#6eeb83",
        "#ffbc42",
        "#ecd444",
        "#ee6352",
        "#9ac2c9",
        "#8acb88",
        "#1be7ff",
    ]
    const myColor = usercolors[Math.floor(Math.random() * usercolors.length)]
    $: awareness.setLocalStateField("user", {name: username, color: myColor})
</script>

<svelte:head>
    <link
        href="https://unpkg.com/tailwindcss@^2.0/dist/tailwind.min.css"
        rel="stylesheet"
    />
</svelte:head>

<div class="flex-col h-screen">
    <div class="flex bg-gray-200">
        <div id="room" class="p-2 font-bold w-60">{title}</div>
        <!--<input id="search" placeholder="Search..." class="m-2 px-3 py-1 w-60">-->
        <div class="flex-1" />
        <div
            id="export"
            class="p-2 cursor-pointer hover:bg-gray-500 text-center"
        >
            📥 Export zip
        </div>
        <div
            style="display: grid;"
            class="hover:bg-gray-500 hover:cursor-pointer w-40"
        >
            <input
                type="file"
                id="import"
                multiple
                style="grid-column: 1; grid-row: 1;"
                class="cursor-pointer"
            />
            <span style="grid-column: 1; grid-row: 1;" class="p-2 text-center"
                >📤 Upload files</span
            >
        </div>
        <div
            id="delete-all"
            class="p-2 cursor-pointer hover:bg-gray-500 text-center"
            on:click={deleteAll}
        >
            💣 Delete all
        </div>
        <div class="dropdown relative">
            <div
                class="bg-gray-300 text-gray-700 font-semibold py-2 px-4 flex place-items-end items-center w-60"
            >
                <span class="mr-1" id="connection-status"
                    >{awarenessStates.length} connected</span
                >
            </div>
            <ul
                class="dropdown-menu absolute hidden text-gray-700 pt-1 bg-gray-100 w-60"
            >
                <div id="users">
                    {#each awarenessStates as [id, state]}
                        <div
                            class="p-2 font-bold"
                            style="color:{state.user.color};"
                        >
                            {#if id == awareness.clientID}
                                <input
                                    type="text"
                                    class="m-2 p-1"
                                    autocomplete="off"
                                    bind:value={username}
                                />
                            {:else}
                                {state.user.name}
                            {/if}
                        </div>
                    {/each}
                </div>
            </ul>
        </div>
    </div>
    <div class="flex flex-col">
        <div class="flex flex-1">
            <div class="flex flex-col bg-gray-300 w-60 h-screen">
                <div class="flex flex-col overflow-y-auto" id="docs">
                    {#each pages as page, i}
                        <div
                            class="doc-button p-2 border-b border-gray-400 flex hover:bg-gray-400 cursor-pointer"
                            data-id={i}
                            on:click={openPage(page)}
                        >
                            {page.get("title").toString()}
                        </div>
                    {/each}
                </div>
                <div
                    id="add-button"
                    class="p-2 hover:bg-blue-400 text-center cursor-pointer"
                    on:click={addPage}
                >
                    ➕ Add page
                </div>
                <div class="flex-1" />
            </div>
            <div class="w-full flex flex-col">
                {#if currentPage}
                    <div id="title">
                        <Editor ytext={currentPage.get("title")} {awareness} />
                    </div>
                    <div id="content" class="flex-grow">
                        <Editor
                            ytext={currentPage.get("content")}
                            {awareness}
                        />
                    </div>
                {/if}
            </div>
        </div>
    </div>
</div>

<style>
    #title {
        border-bottom: 1px solid lightgray;
    }
    #room,
    #title {
        height: 2.8em;
        font-size: 1.1rem;
    }
    #content {
        font-family: monospace;
        height: 10em;
    }
    input[type="file"] {
        opacity: 0.01;
    }
    .dropdown:hover .dropdown-menu {
        display: block;
    }
</style>
