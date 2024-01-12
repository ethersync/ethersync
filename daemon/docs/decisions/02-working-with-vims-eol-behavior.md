---
status: draft
date: 2024-01-12
---
# Working with Vim's EOL behavior

## Context and Problem Statement

Es gibt da einen Bug: Wenn Daemon-seitig eine page mit einem \n endet, und man sie in Vim öffnet, existiert die Newline in Vim nur implizit. Führt dazu, dass zB beim Content "a\n" ein insert(2, "x") vom daemon aus in Vim zu "ax\n" wird.
Um das zu fixen, müssten wir entweder:
- Das newline beim Öffnen der Datei einfügen (und verhindern, dass Vim das mitspeichert), oder
- Mit der eol-Option rumspielen, sodass sie korrekt wiederspiegelt, wie der Dateiinhalt tatsächlich ist (und vermutlich fixeol lokal ausmachen).

Reproduzieren mittels:

-  Buffer hat nur ein "x\n" wenn die Datei geöffnet wird

```
vim.api.nvim_create_user_command("EthersyncInsert", function()
    print(vim.fn.strchars(utils.contentOfCurrentBuffer()))
    local row, col = utils.indexToRowCol(2)
    print(row, col)
    utils.insert(2, "a")
end, {})
```
=> Resultat: Es wird bei "1" eingefügt, weil vim "denkt", dass der Buffer nur aus "x" besteht (wird geclippt in charOffsetToByteOffset).
{Describe the context and problem statement, e.g., in free form using two to three sentences or in the form of an illustrative story.
 You may want to articulate the problem in form of a question and add links to collaboration boards or issue management systems.}

 https://stackoverflow.com/a/16224292


We have to support the following cases:
- file content is "x" (without a newline)
    - when openend with vim, EOL will be false
    - vim writes its buffer and "fixeol" triggers writing an EOL (which is not yet communicated to daemon)
- file content is "x\n" (with a newline)
    - when openend with vim, EOL will be true
- Some other editor removes the implicit \n while vim runs
- Some other editor adds a \n while vim runs
    - this should already work?

<!-- This is an optional element. Feel free to remove. -->
## Decision Drivers

* {decision driver 1, e.g., a force, facing concern, …}
* {decision driver 2, e.g., a force, facing concern, …}
* … <!-- numbers of drivers can vary -->

## Considered Options

* {title of option 1}
* {title of option 2}
* {title of option 3}
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

### {title of option 1}

<!-- This is an optional element. Feel free to remove. -->
{example | description | pointer to more information | …}

* Good, because {argument a}
* Good, because {argument b}
<!-- use "neutral" if the given argument weights neither for good nor bad -->
* Neutral, because {argument c}
* Bad, because {argument d}
* … <!-- numbers of pros and cons can vary -->

### {title of other option}

{example | description | pointer to more information | …}

* Good, because {argument a}
* Good, because {argument b}
* Neutral, because {argument c}
* Bad, because {argument d}
* …

<!-- This is an optional element. Feel free to remove. -->
## More Information

{You might want to provide additional evidence/confidence for the decision outcome here and/or
 document the team agreement on the decision and/or
 define when this decision when and how the decision should be realized and if/when it should be re-visited and/or
 how the decision is validated.
 Links to other decisions and resources might here appear as well.}


Vim `:h eol` writes:

	When 'binary' is off and 'fixeol' is on the value is not used when
	writing the file.  When 'binary' is on or 'fixeol' is off it is used
	to remember the presence of a <EOL> for the last line in the file, so
	that when you write the file the situation from the original file can
	be kept.  But you can change it if you want to.
	See |eol-and-eof| for example settings.
