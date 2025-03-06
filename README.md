# dotpatina

[![Crates.io](https://img.shields.io/crates/v/dotpatina)](https://crates.io/crates/dotpatina)
[![Docs Badge](https://github.com/axis7818/dotpatina/actions/workflows/generate-docs.yaml/badge.svg)](https://camerontaylor.dev/dotpatina/dotpatina/index.html)
[![CI Badge](https://github.com/axis7818/dotpatina/actions/workflows/continuous-integration.yaml/badge.svg?branch=main)](https://github.com/axis7818/patina/actions/workflows/continuous-integration.yaml)
[![CD Badge](https://github.com/axis7818/dotpatina/actions/workflows/continuous-deployment.yaml/badge.svg?branch=main)](https://github.com/axis7818/patina/actions/workflows/continuous-deployment.yaml)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

<p align="center">
  <img src="./dotpatina.png" alt="dotpatina cover image" style="max-width: 256px;">
</p>

dotpatina is a rust application for managing system dotfiles and configuration.

## Installation

`dotpatina` can be installed from its [crate at crates.io](https://crates.io/crates/dotpatina).

```sh
cargo install dotpatina
```

### Verify Installation

```sh
# View dotpatina version
❱ dotpatina --version

# View dotpatina usage info
❱ dotpatina --help
```

## Usage

`dotpatina` takes templated configuration files (using [handlebars templating](https://handlebarsjs.com/guide/)),
rendering configuration files, and applying them to target locations on the file system. This information is provided by
a Patina toml file.

### Patina File

This is an example Patina file for mac development. The full example can be found
at <https://github.com/axis7818/dotfiles>.

```toml
# Metadata fields describe the Patina
name = "axis7818 mac dotfiles"
description = "axis7818 dotfiles for mac"

# Variables are free-form and can be defined for the whole Patina.
# Or, variables can be loaded from separate files from the command line.
[vars]
me.first_name = "Cameron"
me.last_name = "Taylor"

# Finally, A list of files define a template and target file.
# The template is a handlebar template (or plain file) that is processed.
# The target is the system location to store the rendered template.
# Files can also be tagged for filtering when using dotpatina cli commands.

# ZSH
[[files]]
template = "zsh/zshrc"
target = "~/.zshrc"
tags = ["shell"]
[[files]]
template = "zsh/custom/themes/axis7818.zsh-theme"
target = "~/.oh-my-zsh/custom/themes/axis7818.zsh-theme"
tags = ["shell"]

# Tmux
[[files]]
template = "tmux/tmux.conf"
target = "~/.tmux.conf"
tags = ["shell"]

# iTerm
[[files]]
template = "iterm/switch_automatic.py"
target = "~/Library/Application Support/iTerm2/Scripts/AutoLaunch/switch_automatic.py"
tags = ["terminal"]

# Git
[[files]]
template = "git/gitconfig"
target = "~/.gitconfig"
tags = ["git"]
[[files]]
template = "lazygit/config.yml"
target = "~/Library/Application Support/lazygit/config.yml"
tags = ["git"]

# Vim
[[files]]
template = "vim/vimrc"
target = "~/.vimrc"
tags = ["vim"]
[[files]]
template = "jetbrains/ideavimrc"
target = "~/.ideavimrc"
tags = ["vim"]
```

### Variables Files

Variables can be stored in separate toml files. Variables are free-form and overlay on top of the base Patina variables.

This is useful when variables need to change based on the machine Patinas are being applied to.

```toml
me.email = "axis7818@gmail.com"
```

### Template Files

Patina templates are defined using handlebars templates. Or, they can be raw files if no templating is required.

#### Handlebar Template

Templating uses the [Handlebars](https://handlebarsjs.com/guide/) templating language. Templates are rendered using the
variables provided directly in the Patina file and passed as separate variables files.

In this example, `me.email` is pulled from the separate variables file while `me.first_name` and `me.last_name` are
pulled from the Patina file.

`gitconfig`

```hbs
[user]
    email = <{{ me.email }}>
    name = <{{ me.first_name }} {{ me.last_name }}>
[pager]
    branch = false
[core]
    editor = vim
[pull]
    rebase = false
[init]
    defaultBranch = main
[fetch]
    prune = true

```

#### Raw File

Raw files without templating work as well.

`lazygit.config.yml`

```yml
gui:
  showBottomLine: false
  showCommandLog: false
  showPanelJumps: false
  border: rounded
  showNumstatInFilesView: true

customCommands:
  - key: "U"
    command: "git submodule update --init --recursive"
    context: "files, localBranches, status"
    description: "Update submodules"
    stream: true
```

### Render a Patina

`dotpatina` supports rendering Patina files to stdout for previewing.

Provide a path to a Patina toml file that defines files and variables used for rendering. Separate variables toml files
can be provided to overlay variable customizations. Tags can optionally be provided to filter to a subset of files
managed by the Patina.

```sh
dotpatina render <PATINA_TOML_FILE> --vars <VARIABLES_TOML_FILE> [--tags <TAG>]
```

![gif of rendering a patina](./examples/demo/render-patina.gif)

### Applying a Patina

Applying a Patina is how rendered files get written to the file system.

```sh
dotpatina apply <PATINA_TOML_FILE> --vars <VARIABLES_TOML_FILE> [--tags <TAG>]
```

![gif of applying a patina](./examples/demo/apply-patina.gif)

A diff view is presented with each `apply` command to show only lines that will change. This could be due to changing
the template, or using a different set of variables.

```sh
dotpatina apply patina.toml --vars other-vars.toml
```

![gif of updating a patina](./examples/demo/update-patina.gif)
