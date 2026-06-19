# Contributing to Garage

Thank you for helping us make Garage better! All activity is subject to our Code of Conduct.

## Contribution Ideas

Garage is a project with a number of priorities. We love working with the community to improve the editor in ways we haven't thought of (or had time to get to yet!).

In particular we love PRs that are:
- Fixing or extending the docs.
- Fixing bugs.
- Small enhancements to existing features to make them work for more people (platforms, keybindings, actions).

## Sending Changes

The best way to get us to take a look at a proposed change is to send a pull request. We tend to only merge about half the PRs that are submitted. If you'd like your PR to have the best chance of being merged:

- Make sure the change is desired: we're always happy to accept bugfixes, but features should be confirmed with us first if you aim to avoid wasted effort.
- All commit messages and pull request descriptions must be written in English.
- Include a clear description of what you're solving, and why it's important.
- Include tests where applicable.
- Make the PR about one thing only.

### AI Policy

We welcome the use of LLMs for coding, but we hold a high bar for all contributions, and **we expect a human in the loop who genuinely understands the work an LLM produces** on their behalf. For that reason, we **don't accept contributions from autonomous agents**. Pull requests that appear to violate this may be closed, sometimes without notice.

Please write replies to comments and pull request descriptions in your own words.

## UI/UX Checklist

When your changes affect UI, consult this checklist:

- **Accessibility / Ergonomics**: Do all keyboard shortcuts work as intended? Is it usable without a mouse? Do mouse-drag, resize, and scroll work correctly?
- **Responsiveness**: Does the UI scale gracefully on narrow panels and high-DPI displays?
- **Performance**: All user interactions must have instant feedback. Frame compilation and rendering should stay under 8ms (120fps).
- **Consistency**: Layout spacing, colors, and typography must match the global theme configured in `machkit`.

## Things we will (probably) not merge

Typically we don't merge:
- Giant refactorings.
- Non-trivial changes with no tests.
- Stylistic code changes that do not alter any app logic.
- Anything that seems AI-generated without understanding the output.

## Bird's-eye view of Garage

Here are the key modules and directories of the codebase:

- **`src/main.rs`**: Application startup entry point. Handles CLI arguments (such as the `--experimental` flag) and launches the event loop.
- **`src/app/`**: Contains the main winit event loop driver (`mod.rs`), IPC servers (`ipc.rs`), and input handlers (`input/`).
- **`src/renderer/`**: Manages the graphics device, queues, and shaders via WGPU (`wgpu/mod.rs`), and font rasterization / glyph uploading (`atlas.rs`).
- **`src/machkit/`**: The GPU-accelerated UI framework. Implements layout algorithms (`frame.rs`), styling variables (`ui_state.rs`), and widgets (sidebar, terminal, modals, status bar).
- **`src/editor/`**: Represents text editing buffers, cursor selections, keymaps, session history, and configurations.
- **`src/git/`**: Background command execution for fetching branches, status lists, file blame lists, and diffs.
