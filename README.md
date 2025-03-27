<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/jnv-dark.svg">
  <img alt="Text describing the image" src="assets/jnv-light.svg">
</picture>


[![ci](https://github.com/ynqa/jnv/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ynqa/jnv/actions/workflows/ci.yml)

*jnv* is designed for navigating JSON,
offering an interactive JSON viewer and `jq` filter editor.

![jnv.gif](https://github.com/ynqa/ynqa/blob/master/demo/jnv.gif)

Inspired by [jid](https://github.com/simeji/jid)
and [jiq](https://github.com/fiatjaf/jiq).

## Features

- Interactive JSON viewer and `jq` filter editor
  - Syntax highlighting for JSON
  - Use [jaq](https://github.com/01mf02/jaq) to apply `jq` filter
    - This eliminates the need for users to prepare `jq` on their own.
- Capable of accommodating various format
  - Input: File, stdin
  - Data: A JSON or multiple JSON structures
    that can be deserialized with 
    [StreamDeserializer](https://docs.rs/serde_json/latest/serde_json/struct.StreamDeserializer.html),
    such as [JSON Lines](https://jsonlines.org/)
- Auto-completion for the filter
  - Only supports:
    - [Identity](https://jqlang.github.io/jq/manual/#identity)
    - [Object Identifier-Index](https://jqlang.github.io/jq/manual/#object-identifier-index)
    - [Array Index](https://jqlang.github.io/jq/manual/#array-index)
- Hint message to evaluate the filter

## Installation

### Homebrew

See [here](https://formulae.brew.sh/formula/jnv) for more info.

```bash
brew install jnv
```

Or install via Homebrew Tap:

```bash
brew install ynqa/tap/jnv
```

### MacPorts

See [here](https://ports.macports.org/port/jnv/) for more info.

```bash
sudo port install jnv
```

### Nix / NixOS

See [package entry on search.nixos.org](https://search.nixos.org/packages?channel=unstable&query=jnv) for more info.

```bash
nix-shell -p jnv
```

### conda-forge

See [here](https://prefix.dev/channels/conda-forge/packages/jnv) for more info.

```bash
pixi global install jnv
# or
cat data.json | pixi exec jnv
# or
conda install jnv
```

### Docker

Build
(In the near future, the image will be available on something of registries)

```bash
docker build -t jnv .
```

And Run
(The following commad is just an example. Please modify the path to the file you want to mount)

```bash
docker run -it --rm -v $(pwd)/debug.json:/jnv/debug.json jnv /jnv/debug.json
```

### Cargo

```bash
cargo install jnv
```

## Examples

```bash
cat data.json | jnv
# or
jnv data.json
```

## Keymap

| Key | Action |
| :- | :- |
| <kbd>Ctrl + C</kbd> | Exit |
| <kbd>Ctrl + Q</kbd> | Copy jq filter to clipboard |
| <kbd>Ctrl + O</kbd> | Copy JSON to clipboard |
| <kbd>Shift + ↑</kbd>, <kbd>Shift + ↓</kbd> | Switch to another mode |

### Editor mode (default)

| Key | Action |
| :- | :- |
| <kbd>Tab</kbd> | Enter suggestion |
| <kbd>←</kbd> | Move cursor left |
| <kbd>→</kbd> | Move cursor right |
| <kbd>Ctrl + A</kbd> | Move cursor to line start |
| <kbd>Ctrl + E</kbd> | Move cursor to line end |
| <kbd>Backspace</kbd> | Delete character before cursor |
| <kbd>Ctrl + U</kbd> | Clear entire line |
| <kbd>Alt + B</kbd>   | Move the cursor to the previous nearest character within set(`.`,`\|`,`(`,`)`,`[`,`]`) |
| <kbd>Alt + F</kbd>   | Move the cursor to the next nearest character within set(`.`,`\|`,`(`,`)`,`[`,`]`) |
| <kbd>Ctrl + W</kbd>  | Erase to the previous nearest character within set(`.`,`\|`,`(`,`)`,`[`,`]`) |
| <kbd>Alt + D</kbd>   | Erase to the next nearest character within set(`.`,`\|`,`(`,`)`,`[`,`]`) |

#### Suggestion in Editor (after <kbd>Tab</kbd>)

| Key | Action |
| :- | :- |
| <kbd>Tab</kbd>, <kbd>↓</kbd> | Select next suggestion |
| <kbd>↑</kbd> | Select previous suggestion |
| Others | Return to editor |

### JSON viewer mode

| Key | Action |
| :- | :- |
| <kbd>↑</kbd>, <kbd>Ctrl + K</kbd> | Move up |
| <kbd>↓</kbd>, <kbd>Ctrl + J</kbd> | Move down |
| <kbd>Ctrl + H</kbd> | Move to last entry |
| <kbd>Ctrl + L</kbd> | Move to first entry |
| <kbd>Enter</kbd> | Toggle fold |
| <kbd>Ctrl + P</kbd> | Expand all |
| <kbd>Ctrl + N</kbd> | Collapse all |

## Usage

```bash
JSON navigator and interactive filter leveraging jq

Usage: jnv [OPTIONS] [INPUT]

Examples:
- Read from a file:
        jnv data.json

- Read from standard input:
        cat data.json | jnv

Arguments:
  [INPUT]  Optional path to a JSON file. If not provided or if "-" is specified, reads from standard input

Options:
  -c, --config <CONFIG_FILE>             Path to the configuration file.
      --default-filter <DEFAULT_FILTER>  Default jq filter to apply to the input data
  -h, --help                             Print help (see more with '--help')
  -V, --version                          Print version
```

## Stargazers over time
[![Stargazers over time](https://starchart.cc/ynqa/jnv.svg?variant=adaptive)](https://starchart.cc/ynqa/jnv)
