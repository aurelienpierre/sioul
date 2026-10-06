---
description: Notes in Sioul - your own folder of Markdown files, the same as an Obsidian vault or your Nextcloud Notes, with links both ways, pictures, PDFs, sounds and audio memos.
---

# Notes

Your notes are your own folder of Markdown files. Sioul reads it as Obsidian reads a vault, and links to it; it never owns it. You can keep writing in it with any other program.

## The same folder as Obsidian and Nextcloud Notes

Sioul's notes are fully compatible with an Obsidian vault and with Nextcloud Notes. Choose your vault, or the **Notes** folder that Nextcloud Notes keeps in your Nextcloud, as Sioul's notes folder, and keep using them side by side: the same files, nothing imported, nothing converted.

- **As Obsidian reads them**: `[[wikilinks]]`, with `#heading` and `|text shown`, found by name as Obsidian finds them (the same folder first, then the shortest path, then aliases); `![[pictures]]` shown in place; Markdown links; `#tags`; front matter (title, tags, aliases); `- [ ]` checkboxes; and the notes that link back. A note moved to the trash goes to the vault's `.trash`, as in Obsidian. Renamed in Sioul, a note takes the links of the other notes along.
- **As Nextcloud Notes writes them**: its categories are folders, and its notes, saved as `.txt` by default or `.md`, are read and edited as the Markdown they are, each keeping its own name and extension. New notes made in Sioul are `.md`, which Nextcloud Notes shows too, on the web and in its phone apps.
- **What Sioul adds stays plain Markdown**: a note tied to a task, a message or an event holds an ordinary link (`sioul:task/…`, `mid:…`), or a line of front matter, which other programs show as a link or simply leave alone.
- **Never its own**: Sioul writes only the notes you write or make from it, and never reads Obsidian's `.obsidian` folder, the `.trash`, `.git` or any hidden folder.

<figure markdown="span">
  [![The Notes page: New note and the other buttons above a list of every note, each with its folder; a note open in the middle, its path and tags under its title, its text with links and a picture of a sketch; on the right, what is tied to it (tasks, a message, other notes) and its lines not ticked, each with a button that makes it a task.](../assets/screens/notes.png){ loading=lazy }](../assets/screens/notes.png "Open the picture at full size")
  <figcaption>A note, read, with what is tied to it on the side.</figcaption>
</figure>

## Your notes folder

Chosen in **Settings ▸ Your folder and sharing ▸ The notes folder**. Beside your notes, the same folder keeps your projects, budgets, papers and scanned letters, so that they all travel together when the folder is synced to your other devices. Where no sync carries it, Sioul's sharing can carry them, if you switch them on there ([Sharing](sharing.md)).

## Finding a note

Search comes first: **Search the notes**. Below it, the notes **changed lately**, then **every note**, as one **List** or as **Folders**, folded until opened. Sioul remembers which one you chose. No filing is asked of you.

## Reading and writing

A note opens to be read. Its links are followed with a click, wherever they lead: another note, a task, a message, an event.

- **Edit** switches to writing; **Save** keeps it.
- `[[Another note]]` links to a note by its name, wherever it is in the folder. Markdown links, `#tags` and front matter work too.
- Each note shows what links to it, and what it is tied to: tasks, messages, events, people, projects.
- **Aa** sets the font, its size and the space between lines, for reading.
- **Open with…** opens the note in another program.

If the note changed elsewhere while you were writing (on another device, by a sync app, in another program), your version is never saved over it: it is kept beside it, as "Plan (conflict 2026-10-05 21.50)", and stays open; the status line says so. The other version keeps the name. On a phone, **Back** keeps the note, saved, and closes it.

A line such as `- [ ] Ask for the certificate`, not ticked, is one click from becoming a task: **Make it a task**. The task stays tied to its note.

## New notes

**New note**, or **New ▾ ▸ A note**, asks for its title. A note can also be made from a message (it starts with who wrote it), from an event (dated, its guests listed, then **Send to the guests**), or from a task.

Notes made from mail and events go into one folder of your notes, set in the Notes page's ⚙: **New notes go into**.

A right click on a folder: **New note here…**, **New folder inside…**, **Rename the folder…**, **Take out this empty folder**.

## Pictures, PDFs and sounds

They are notes too, listed beside the Markdown files, and opened in place: a picture fitted to the page, a PDF's pages, a sound with play, pause and where it is. They can be tied to tasks and projects like any note.

`![[scan.png]]` shows the picture inside a note.

**Record an audio memo** records into the `memos` folder of your notes, and opens it as a note. The microphone is open only while it records.

## Renaming and deleting

A right click on a note:

- **Rename…**: the notes that link to it, and the tasks tied to it, follow the new name.
- **Move to the trash**: the note goes to the folder's `.trash`, as Obsidian does, with **Undo**.

## On several devices {#on-several-computers}

Your notes travel with their folder's own sync: Nextcloud, Dropbox, Syncthing. Where no sync carries the folder, a phone's for one, Sioul's sharing can carry them, file by file, once you switch **Notes** on in it. See [Sharing between your devices](sharing.md).

A linked note that has not arrived yet shows faded, with "Not on this device yet: it may still be syncing".
