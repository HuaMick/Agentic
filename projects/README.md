# projects/

User-space directory for projects that this Agentic harness is being used on. Each sub-directory here is a project the harness drives work against — not part of the harness itself.

## What goes here

Checked-out or symlinked project repositories. These are the *targets* of orchestration: the codebases that stories, agents, and verification runs operate on. The harness reads them, plans work against them, and records evidence about them, but does not own their code.

## Layout

One sub-directory per project, named after the project:

```
projects/
  my-web-app/        # checked out or symlinked
  some-other-repo/
```

Whether a project is a nested git clone, a git submodule, a symlink to somewhere else on disk, or a worktree is up to the user. The harness does not prescribe.

## Scope

This is **user space.** The harness provides the conventional location and nothing else:

- No required files, no schema, no naming rules beyond "one directory per project."
- Nothing here is authoritative to the harness.
- `.gitignore` policy for the contents is the user's call.

## Phase 1 status

Empty. First projects will be added as the harness matures to the point where it can meaningfully drive external work.
