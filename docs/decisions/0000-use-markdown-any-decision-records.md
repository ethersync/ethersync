# Use Markdown Any Decision Records

## Context and Problem Statement

We want to record any decisions made in this project independent whether decisions concern the architecture ("architectural decision record"), the code, or other fields.
Which format and structure should these records follow?

## Considered Options

* [MADR](https://adr.github.io/madr/) 3.0.0 – The Markdown Any Decision Records

## Decision Outcome

Chosen option: "MADR 3.0.0", because

* It was the first template that I found out how to add to the project
    * easy to copy via npm `madr`.
* It's probably easier to just start with one template and adapt over time
 rather than overthinking the template decision.
    * It shouldn't be too hard to switch to a new format, old decision can just stay in the old one.
