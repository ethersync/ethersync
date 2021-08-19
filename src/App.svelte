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
    import sanitize from "sanitize-filename"

    import * as Y from "yjs"
    import {WebsocketProvider} from "y-websocket"
    import {IndexeddbPersistence} from "y-indexeddb"

    let ydoc,
        ypages,
        persistence,
        pages = []

    let provider, awareness
    let awarenessStates = []
    let connectionStatus = "unknown"

    function updatePages() {
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
    }

    $: if (title) {
        ydoc = new Y.Doc()
        ypages = ydoc.getArray("pages")

        if (persistence) {
            persistence.destroy()
        }
        persistence = new IndexeddbPersistence(title, ydoc)

        pages = ypages.toArray()
        ypages.observeDeep(updatePages)

        if (provider) {
            provider.disconnect()
            provider.destroy()
        }
        provider = new WebsocketProvider(
            location.origin.replace(/^http/, "ws"),
            `${title}`,
            ydoc,
        )
        provider.on("status", (event) => {
            connectionStatus = event.status
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

        searchTerm = ""
        currentPage = ypage
    }

    let currentPage = null

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

    let searchTerm = ""

    $: if (searchTerm.length > 0) {
        let matchingPages = [...pages]
            .sort(
                (a, b) =>
                    a.get("title").toString().length -
                    b.get("title").toString().length,
            )
            .filter((p) =>
                p
                    .get("title")
                    .toString()
                    .toLowerCase()
                    .includes(searchTerm.toLowerCase()),
            )
        if (matchingPages.length > 0) {
            let targetPage = matchingPages[0]
            if (targetPage != currentPage) {
                currentPage = targetPage
            }
        }
    }

    function exportZip() {
        var zip = new JSZip()
        for (const doc of ypages) {
            const originalTitle = doc.get("title").toString()
            let title = sanitize(originalTitle)
            if (title == "") {
                const hashCode = (s) =>
                    s
                        .split("")
                        .reduce(
                            (a, b) => ((a << 5) - a + b.charCodeAt(0)) | 0,
                            0,
                        )
                title = "invalid_title_" + hashCode(originalTitle) // Quick hack. :/
            }
            const content = doc.get("content").toString()
            zip.file(title, content)
        }
        zip.generateAsync({type: "blob"}).then((content) => {
            saveAs(content, `${title}.zip`)
        })
    }
</script>

<svelte:head>
    <title>{title}</title>
</svelte:head>

{#if title}
    <div class="flex flex-col h-full">
        <div class="flex flex-wrap bg-gray-200">
            <div id="room" class="p-2 font-bold flex items-center">
                🍃 {title}
            </div>
            <input
                type="search"
                class="m-2 px-3 py-1 w-60"
                bind:value={searchTerm}
                placeholder="Find page..."
                list="pages"
            />
            <datalist id="pages">
                {#each pages as page}
                    <option>{page.get("title").toString()}</option>
                {/each}
            </datalist>
            <div
                class="p-2 hover:bg-gray-500 text-center cursor-pointer flex items-center"
                on:click={addPage}
                title="Add page"
            >
                ➕
            </div>
            <div class="flex-1" />
            <div
                class="p-2 cursor-pointer hover:bg-gray-500 text-center flex items-center"
                on:click={exportZip}
                title="Export zip"
            >
                📥
            </div>
            <div
                style="display: grid;"
                class="hover:bg-gray-500 hover:cursor-pointer w-10"
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
                    class="p-2 text-center flex items-center justify-center"
                    title="Upload files">📤</span
                >
            </div>
            <div
                id="delete-all"
                class="p-2 cursor-pointer hover:bg-gray-500 text-center flex items-center"
                on:click={deleteAll}
                title="Delete all"
            >
                💣
            </div>
            <div class="dropdown relative flex">
                <div
                    class="{connectionStatus == 'connected'
                        ? 'bg-gray-300'
                        : 'bg-red-300'} text-gray-700 font-semibold py-2 px-4 place-items-end items-center flex w-40"
                >
                    <span class="mr-1">
                        {#if connectionStatus == "connected"}
                            {awarenessStates.length} connected
                        {:else}
                            {connectionStatus}
                        {/if}
                    </span>
                </div>
                <ul
                    class="dropdown-menu absolute top-12 hidden z-10 text-gray-700 pt-1 bg-gray-100 w-40"
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
        <div class="flex flex-col flex-1">
            {#if currentPage}
                <div class="flex border-b border-gray-300">
                    <div id="title" class="flex-grow">
                        <Editor ytext={currentPage.get("title")} {awareness} />
                    </div>
                    <div
                        class="p-2 hover:bg-red-500 flex text-center items-center"
                        on:click={deletePage(currentPage)}
                    >
                        🗑️
                    </div>
                </div>
                <div id="content" class="flex-grow">
                    <Editor ytext={currentPage.get("content")} {awareness} />
                </div>
            {/if}
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
    #room,
    #title {
        height: 2.9em;
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
