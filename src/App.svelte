<script>
    var title, enterTitle
    function hashchange() {
        title = window.location.hash.substring(1)
        if (title.length == 0) {
            title = null
        }
    }
    window.addEventListener("hashchange", hashchange)
    hashchange()
    $: if (title) {
        window.location.hash = title
    }

    import Editor from "./Editor.svelte"

    import JSZip from "jszip"
    import {saveAs} from "file-saver"

    import * as Y from "yjs"
    import {WebrtcProvider} from "y-webrtc"
    import {IndexeddbPersistence} from "y-indexeddb"

    let ydoc, ypages, persistence, pages

    let provider, awareness
    let awarenessStates = []

    $: if (title) {
        ydoc = new Y.Doc()
        ypages = ydoc.getArray("pages")

        if (persistence) {
            persistence.destroy()
        }
        persistence = new IndexeddbPersistence(title, ydoc)

        pages = ypages.toArray()
        ypages.observeDeep(() => {
            pages = ypages.toArray().sort(function (first, second) {
                const nameA = first.get("title").toString().toLowerCase()
                const nameB = second.get("title").toString().toLowerCase()
                if (nameA < nameB) {
                    return -1
                }
                if (nameA > nameB) {
                    return 1
                }
                return 0
            })
        })

        if (provider) {
            provider.disconnect()
            provider.destroy()
        }
        provider = new WebrtcProvider(`etherwiki-${title}`, ydoc, {
            signaling: [
                "wss://signaling.yjs.dev",
                "wss://y-webrtc-signaling-eu.herokuapp.com",
                "wss://y-webrtc-signaling-us.herokuapp.com",
            ],
        })
        awareness = provider.awareness

        awareness.on("change", () => {
            awarenessStates = [...awareness.getStates()]
        })

        currentPage = null
    }

    const addPage = () => {
        const ypage = new Y.Map()

        const ytitle = new Y.Text()
        ytitle.insert(0, "New Page")
        ypage.set("title", ytitle)

        const ycontent = new Y.Text()
        ypage.set("content", ycontent)

        ypages.push([ypage])

        currentPage = ypage
        console.log(awareness)
    }

    let currentPage = null
    const openPage = (page) => {
        currentPage = page
    }

    /*
    $: if (currentPage) {
        console.log(currentPage)
        awareness.setLocalStateField(
            "page",
            currentPage.get("title").toString(),
        )
    }
    */

    let deletePage = (page) => {
        if (confirm(`Really delete '${page.get("title")}'?`)) {
            currentPage = null
            let i = ypages.toArray().indexOf(page)
            ypages.delete(i)
        }
    }

    const deleteAll = () => {
        if (confirm(`Really delete all pages?`)) {
            currentPage = null
            ypages.delete(0, ypages.length)
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

    $: if (awareness && username) {
        awareness.setLocalStateField("user", {name: username, color: myColor})
    }

    let files

    $: if (files) {
        Array.from(files).forEach((f) => {
            var reader = new FileReader()
            reader.onload = ((file) => {
                return function (e2) {
                    var existingPage = ypages
                        .toArray()
                        .find((p) => p.get("title") == file.name)
                    if (existingPage) {
                        var content = existingPage.get("content")
                        content.delete(0, content.length)
                        content.insert(0, e2.target.result)
                    } else {
                        const newDoc = new Y.Map()
                        const title = new Y.Text()
                        title.applyDelta([{insert: file.name}])
                        const content = new Y.Text()
                        content.applyDelta([{insert: e2.target.result}])
                        newDoc.set("title", title)
                        newDoc.set("content", content)
                        ypages.push([newDoc])
                    }
                }
            })(f)
            reader.readAsText(f)
        })
        files = null
    }

    function exportZip() {
        var zip = new JSZip()
        for (const doc of ypages) {
            const title = doc.get("title").toString()
            const content = doc.get("content").toString()
            zip.file(title, content)
        }
        zip.generateAsync({type: "blob"}).then((content) => {
            saveAs(content, `${title}.zip`)
        })
    }
</script>

<svelte:head>
    <link
        href="https://unpkg.com/tailwindcss@^2.0/dist/tailwind.min.css"
        rel="stylesheet"
    />
    <title>{title}</title>
</svelte:head>

{#if title}
    <div class="flex-col h-screen">
        <div class="flex bg-gray-200">
            <div id="room" class="p-2 font-bold w-60">🍃 {title}</div>
            <!--<input id="search" placeholder="Search..." class="m-2 px-3 py-1 w-60">-->
            <div class="flex-1" />
            <div
                class="p-2 cursor-pointer hover:bg-gray-500 text-center"
                on:click={exportZip}
            >
                📥 Export zip
            </div>
            <div
                style="display: grid;"
                class="hover:bg-gray-500 hover:cursor-pointer w-40"
            >
                <input
                    type="file"
                    multiple
                    bind:files
                    style="grid-column: 1; grid-row: 1;"
                    class="cursor-pointer"
                />
                <span
                    style="grid-column: 1; grid-row: 1;"
                    class="p-2 text-center">📤 Upload files</span
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
                    class="dropdown-menu absolute hidden z-10 text-gray-700 pt-1 bg-gray-100 w-60"
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
                                class="border-b border-gray-400 flex hover:bg-gray-400
                                {currentPage == page
                                    ? 'bg-gray-500 hover:bg-gray-500'
                                    : ''} cursor-pointer"
                                data-id={i}
                                on:click={openPage(page)}
                            >
                                <div class="flex-grow p-2">
                                    {page.get("title").toString()}
                                </div>
                                {#if currentPage == page}
                                    <div
                                        class="p-2 hover:bg-red-500"
                                        on:click={deletePage(page)}
                                    >
                                        ×
                                    </div>
                                {/if}
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
                            <Editor
                                ytext={currentPage.get("title")}
                                {awareness}
                            />
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
{:else}
    <div class="w-80 my-10 mx-auto">
        Enter a name for your EtherWiki: <input
            bind:this={enterTitle}
            class="bg-gray-100 p-2"
            on:keydown={(e) => {
                if (e.keyCode == 13) {
                    title = e.target.value
                }
            }}
        />
        <button
            class="bg-green-500 p-2"
            on:click={(e) => (title = enterTitle.value)}>OK</button
        >
    </div>
{/if}

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
