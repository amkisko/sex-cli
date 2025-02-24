#!/bin/bash
set -e

# Create completions directory if it doesn't exist
mkdir -p ~/.zsh/completions

# Generate zsh completions
sex completion zsh > ~/.zsh/completions/_sex

# Add completions directory to fpath if not already added
if ! grep -q 'fpath=(~/.zsh/completions $fpath)' ~/.zshrc; then
    echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc
    echo 'autoload -U compinit; compinit' >> ~/.zshrc
fi

echo "Zsh completions installed. Please restart your shell or run:"
echo "  source ~/.zshrc" 