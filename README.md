# jj-gpc

✨ LLM-based bookmark (Git branch) name creation for [Jujutsu VCS][jj]

[jj]: https://github.com/martinvonz/jj

Jujutsu is a modern, Git-compatible VCS. It supports (and I primarily use) anonymous branches for development. Since Git, and thus all Git-based “forges” (GitHub, GitLab, Bitbucket, etc.) require branch names, though, you need to give a branch name when you push your changes for others to work with.

Jujutsu has native support making this easy. If you run `jj git push --change <change ID>`,it creates a Jujutsu bookmark (which it maps to Git branches) and then pushes that newly created bookmark. The bookmark names it creates are of the form `push-<change id>`, though. Those are not especially attractive to most collaborators!

This tiny tool is one “solution”: it generates a bookmark name based on the messages associated with the changes you tell it to use—by default, `trunk()..@`, or “everything between my current working copy and whatever the ‘trunk’ is for this project (usually `main` or `master`”).

`gpc` is short for `git push change`.

## Installation

Prerequisites: a relatively recent version of Rust. (I built it with Rust 1.82, but it probably works with versions quite a bit earlier than that!)

- Clone the repo.

    With Jujutsu:

    ```sh
    jj git clone https://github.com/chriskrycho/jj-gpc
    ```

    With Git:

    ```sh
    git clone https://github.com/chriskrycho/jj-gpc.git
    ```

- Install it with Cargo:

    ```sh
    cargo install --path . --locked
    ```

- Install and run [ollama][o].

- Fetch [the `llama3.2` model][model]:

    ```sh
    ollama fetch llama3.2
    ```

[o]: https://ollama.com
[model]: https://ollama.com/library/llama3.2

That’s it; now you can run `jj-gpc` to do this.

## Options

- `-r`/`--revision`: the revset to use to generate the bookmark name. At the moment, this is not used for which changes to use to generate the message; the bookmark is always created at `@`.
- `-p`/`--prefix`: apply a prefix before the generated bookmark name. For example, `jj-gpc -p chriskrycho` would produce a name like `chriskrycho/did-some-stuff`, instead of just `did-some-stuff`.
- `--dry-run`: generate a branch name but neither create the bookmark nor push it.
