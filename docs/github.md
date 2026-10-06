# GitHub: issues and pull requests as tasks

For those who work on GitHub. Off unless asked: the last line of the task settings (Tasks ⚙, "Code"), "GitHub issues and pull requests as tasks". Nothing else shows until then.

## What comes
The open issues and pull requests that are yours, found by GitHub's search across all of GitHub (public repositories, and the private ones your token reaches):

| | Search | On by default |
|---|---|---|
| Assigned to you | `is:open is:issue assignee:@me archived:false`, then the same with `is:pr` | yes |
| Your review asked | `is:open is:pr user-review-requested:@me archived:false` | yes |
| Opened by you | `is:open is:issue author:@me archived:false`, then `is:pr` | no |
| Mentioning you | `is:open is:issue mentions:@me archived:false`, then `is:pr` | no |

Issues and pull requests are searched apart: with a fine-grained token (or a GitHub App's user token), GitHub refuses a search that could return both, with a 422 ("Query must include 'is:issue' or 'is:pull-request'"; [GitHub's REST documentation](https://docs.github.com/en/rest/search/search#search-issues-and-pull-requests)). Until 6 October 2026 Sioul asked them together, and that refusal stopped the whole sync, so no task came. A refused request now says GitHub's own reason in the status line. When one search fails, the whole sync waits for the next round: tasks are never closed on a partial answer.

Each becomes a task in a list "GitHub" kept on this device only (`calendars/local/github`), never sent to a server: UID `github:owner/repo#12`, its title, a link to its page, why it is yours (`X-SIOUL-GITHUB-REASON`). An issue found by two searches comes once, with the strongest reason (assigned, then review, then opened, then mentioned). Your projects' routes place it in a project as they place mail: the issue is matched as GitHub's mail about it would be, from `notifications@github.com` with the subject `[owner/repo] Title (Issue #12)`; a route on the words `[owner/repo]` takes a repository's issues.

Every thirty minutes while Sioul is open, and at "Sync now". Answers are asked with their ETag: an unchanged one costs nothing on GitHub's limits.

## What GitHub owns, and what you do
- GitHub's title and link are written again when they change.
- Open, done or dropped is written only when GitHub's state changes (`X-SIOUL-GITHUB` keeps what it said last): an issue you marked done or dropped here stays so while it stays open there; closed there, it is done here (dropped when closed as not planned or a duplicate); reopened there, it comes back.
- What you add here stays: a date, a length, steps, a kind, a case.
- An issue no longer found (unassigned, review given): Sioul asks GitHub for it; closed, it is done here; still open and unchanged for an hour (the search may lag behind a change), it is dropped here; it never disappears without a trace.
- Nothing is written to GitHub.

## Its mail
GitHub's notification mail is tied to its issue's task, and to that task's project, as it arrives: the issue is the thread's root, read from `In-Reply-To`, else `Message-ID` (`<owner/repo/issues/12@github.com>`, `<owner/repo/pull/7/c345@github.com>`, `<owner/repo/issue/5/issue_event/6@github.com>`). Mail about an issue that is not yours makes nothing. The reply address GitHub puts in `To` (replying to it posts as you) is never shown, logged or exported.

## The token
A fine-grained personal access token, kept in the system keyring ("GitHub token"):
1. "Make a token on GitHub" opens GitHub's page with its name and rights filled in: Issues read, Pull requests read, Metadata read, a year.
2. Choose the repositories (all of yours, or some), make it, and paste it in Sioul ("Keep").

It reads, nothing more. GitHub removes a token unused for a year; at its end, the status line says GitHub refused it, and a new one goes in the same place. A token reaches one owner's private repositories (yours, or one organisation's); public repositories everywhere are always read.

## What was tested
Against a stand-in of GitHub's API (`tools/github-stand-in.py`): an assigned issue and a review request become tasks; a task whose issue was closed there is done here with GitHub's date; one no longer yours is dropped. The mail's thread reading and what GitHub changes in a task are unit tests. Not tested against GitHub itself, nor on real notification mail.

## Files
- `crates/sioul-core/src/github.rs`: issues as tasks, what GitHub changes, the mail's thread.
- `crates/sioul-sync/src/github.rs`: the API, the token, the ETag cache (`~/.local/state/sioul/github-cache.json`).
- `crates/sioul-app/src/github.rs`: every thirty minutes, the list, the mail tied.
