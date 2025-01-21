# Sentry Explorer CLI (sex-cli)

A command-line interface tool for exploring Sentry issues and data, inspired by GitHub CLI's user experience. This project is generated and maintained with [Cursor Composer](https://cursor.sh), and all changes should be implemented through it to maintain consistency and quality.

## Features

- Multi-organization support with encrypted authentication
- Issue listing and exploration
- Interactive issue details view
- Real-time issue monitoring
- Secure token and project caching
- Organization management
- Cross-platform TUI interface

## Requirements

- Rust (stable)
- libsodium (for encryption)
- System keyring support
  - Linux: libsecret
  - macOS: Keychain
  - Windows: Credential Manager

## Installation

### From Source

1. Install Rust (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Install system dependencies:
   - Ubuntu/Debian:
     ```bash
     sudo apt-get update
     sudo apt-get install -y libsodium-dev libsecret-1-dev
     ```
   - macOS:
     ```bash
     brew install libsodium
     ```

3. Build and install:
   ```bash
   cargo install --path .
   ```

## Usage

### Organization Management
```bash
# List organizations
sex org list

# Add organization
sex org add <name> <slug>

# Login to organization
sex login <org> <token>
```

### Issue Management
```bash
# List issues
sex issue list

# View issue details
sex issue view <id>

# Monitor issues in real-time
sex monitor <org> [project]
```

## Development

> **Important**: This project uses Cursor Composer for development. Please make all changes through the Cursor IDE to ensure consistent code quality and documentation.

### Setup

1. Install [Cursor](https://cursor.sh)
2. Clone the repository
3. Open the project in Cursor
4. Use Cursor Composer for implementing changes

### Project Structure

```
.
├── src/
│   ├── main.rs           # Entry point
│   ├── commands.rs       # CLI commands
│   ├── config.rs         # Configuration
│   ├── sentry.rs         # API client
│   ├── tui.rs           # TUI components
│   ├── issue_viewer.rs   # Issue viewer
│   └── dashboard.rs      # Monitoring
├── doc/                  # Documentation
│   ├── architecture.md   # Architecture decisions
│   └── development.md    # Development guide
└── .github/             # GitHub configuration
```

### Testing

```bash
# Run all tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run lints
cargo clippy

# Generate coverage report
cargo llvm-cov
```

### CI/CD

The project uses GitHub Actions for:
- Cross-platform testing (Linux, macOS)
- Code formatting checks
- Linting
- Test coverage reporting
- Release builds

## Contributing

1. All changes must be implemented through Cursor Composer
2. Follow the development guide in `doc/development.md`
3. Ensure tests pass and coverage is maintained
4. Submit a pull request

## License

MIT License 