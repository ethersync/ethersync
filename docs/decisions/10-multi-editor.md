---
status: draft
date: 2024-09-01
---
# How to handle opening the same file in multiple editors?

## Context and Problem Statement

This is related to ADR-04: When we can open the same file in multiple editors on a local system, how can we make sure that they are in sync?

Problematic example: Open a file with editor 1, and change its content, but don't save. Then, open the file with editor 2. The editor will read the file from disk, and get an older version of the content. There's no mechanism to prevent this.

## Decision Drivers

Our solution should:

* "feel natural", and do what users would expect
* always give correct results/never mangle buffer contents

## Considered Options

* Editor sends along its content with "open", and is rejected when the content isn't correct
* Editor sends along its content with "open", and gets sent updates when the content isn't correct
* Daemon sends "real content" to editor when it connects, editor then updates to it
* Only one editor can own a file, other "open"s fail
* Daemon always writes CRDT content to file, plugins must make sure that editors aren't unhappy about that

## Decision Outcome

Chosen option: "{title of option 1}", because
{justification. e.g., only option, which meets k.o. criterion decision driver | which resolves force {force} | … | comes out best (see below)}.

## Pros and Cons of the Options

### Editor sends along its content with "open", and is rejected when the content isn't correct

LSP also sends along the file content along with an "open" message (TODO: What does LSP call the message?).
TODO: Find out why LSP does this.

* Good, because it seems like the simplest solution, while still allowing some multi-editor testing.
* Bad, because it interrupts a natural workflow. It forces users to save the file in another editor before opening it with an additional one.
* Bad, because the file content that has to be sent along could be very big.

### Editor sends along its content with "open", and gets sent updates when the content isn't correct

This is a bit like we update the CRDT on Ethersync start if there's a difference. But the other way around: Here, we update the buffer contents to match the CRDT.

* Good, because this seems to lead to a natural, uninterrupted workflow.
* Good, because it only requires a small protocol extension.
* Neutral, we have to be careful with edge cases here (where this initial update happens in parallel to a peer update, for example).
* Bad, because the file content that has to be sent along could be very big.
* Bad, because the diff that's sent back could be big and complex.

### Daemon sends "real content" to editor when it connects, editor then updates to it

Here, we take the CRDT state as the source of truth, and directly feed it into the editor.

Note: As mentioned in ADR-04, it seems as if this would enable local "editors" that don't have/don't need access to the filesystem, like browser-based ones? (But how realistic is it that these could make the `ethersync client` connection?) This could also be supported in the previous solution, if the editor sends an empty content.

* Good, because this seems to lead to a natural, uninterrupted workflow.
* Bad, because this adds complexity to the protocol: There needs to be an initialization message immmediately after the connect, which the editor needs to wait for.
* Bad, because the file content that has to be sent along could be very big.

### Only one editor can own a file, other "open"s fail

* Neutral, it would avoid incorrect states.
* Neutral, it prevents multi-editor usage – but without Ethersync, this is also not something that users could do.

### Daemon always writes CRDT content to file, plugins must make sure that editors aren't unhappy about that

* Good, because this allows external tools to see the correct, current file content.
* Bad, because it's more work for the plugins.
* Bad, because it's unclear if all editors can be "made happy" with outside writes while they have the file open.

## More Information

Why support opening a file in multiple editors at once anyway? There might be these reasons:

- It simplifies developing/testing editor plugins a lot, because you don't have to spin up two connected daemons.
- If users accidentally open a file multiple times, instead of leading to a problematic conflict (as it would happen without Ethersync), it now smoothly integrates, which is kind of cool!
- It allows users to work on the same file with editors on multiple screens. This could be helpful when giving a presentation or something?

The first reason seems like the most important one.

Currently, we ignore changes to files made by external tools while the editors have ownership. Integrating those would be an additional step, and requires reasoning about how to calculate the diff correctly...
