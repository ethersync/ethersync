---
status: draft
date: 2024-01-12
---
# Working with Vim's EOL behavior

## Context and Problem Statement

When a file ends with a newline and is opened in Vim, the newline exists in Vim only implicitly (the 'eol' option will be set). This leads to an issue where, for example, with the content "a\n", an insert(2, "x") from the daemon in Vim refers to a character position that's not actually there.

In addition, when the 'fixeol' option is set, and both 'eol' and 'binary' are false, Vim will add a trailing newline to content that has no newline on save. This insertion needs to be reflected in the shared content.

## Considered Options

* Always force 'eol' and 'fixeol' off.
* When 'fixeol' is on, send the fixes a Vim write would make to the daemon immediately.
* … <!-- numbers of options can vary -->

## Decision Outcome

Chosen option: "{title of option 1}", because
{justification. e.g., only option, which meets k.o. criterion decision driver | which resolves force {force} | … | comes out best (see below)}.

<!-- This is an optional element. Feel free to remove. -->
### Consequences

* Good, because {positive consequence, e.g., improvement of one or more desired qualities, …}
* Bad, because {negative consequence, e.g., compromising one or more desired qualities, …}
* … <!-- numbers of consequences can vary -->

<!-- This is an optional element. Feel free to remove. -->
## Validation

{describe how the implementation of/compliance with the ADR is validated. E.g., by a review or an ArchUnit test}

<!-- This is an optional element. Feel free to remove. -->
## Pros and Cons of the Options

### Always force 'eol' and 'fixeol' off.

Always force 'eol' and 'fixeol' off. When opening a file, and it contains a trailing \n, insert it in the buffer (but set 'eol' off).

* Good, because this allows us to display cursors in those empty new lines.
* Good, because it seems simple.
* Bad, because Vim will now always show line breaks at end of regular files, which is not what people are used to.

### When 'fixeol' is on, send the fixes a Vim write would make to the daemon immediately.

In our plugin, look at 'fixeol', to determine whether Vim will ever meddle with the file. If it is on (and 'eol' is false), already send an inserted \n to the daemon at the earliest possibility. This is as if Vim writes as soon as it openes the file. Set 'eol' to true in that case.

If any operation leaves us with a (real) content without a newline in the end, set 'eol' to false. But if 'fixeol' is true, immediately send the "fixing" \n to the daemon again.

* Good, because from Vim's point of view, the file behaves normally.
* Bad, because a cursor in a new, empty line can't be represented (probably?). But we could put it at the end of the previous line.
* Neutral, because it can lead to strange behavior when other clients delete the trailing newline. Our plugin will immediatel re-add it. But that might be what's to be expected.

<!-- This is an optional element. Feel free to remove. -->
## More Information

Vim `:h eol` writes:

	When 'binary' is off and 'fixeol' is on the value is not used when
	writing the file.  When 'binary' is on or 'fixeol' is off it is used
	to remember the presence of a <EOL> for the last line in the file, so
	that when you write the file the situation from the original file can
	be kept.  But you can change it if you want to.
	See |eol-and-eof| for example settings.

Note: The `contentOfCurrentBuffer` function should probably always look at 'eol' to return the real, implied content.

Note: Vim doesn't seem to set 'eol' to false when the file is completely empty.

Observation:

- Have a file not ending in \n.
- Open it in Vim. 'eol' will be off.
- Verify 'fixeol' is on.
- Write.
- File now ends in \n.
- But 'eol' is still on. :O So we can't really use it as a "does the last line have an implicit newline" indicator in this case.
- But in our solution, we can toggle that option on and off and assume it does.
