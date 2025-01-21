# Architecture Decisions

## 1. Language Choice - Rust

**Decision**: Use Rust as the primary programming language.

**Rationale**:
- Memory safety with zero-cost abstractions
- Strong type system and ownership model
- Rich ecosystem with crates.io
- Cross-platform compilation support
- Built-in package manager (Cargo) and testing framework
- Excellent error handling with Result type
- Smaller binary size through optimization

## 2. Authentication Storage

**Decision**: Use libsodium (via sodiumoxide) for token encryption and keyring for key storage.

**Rationale**:
- Industry-standard encryption library
- Secure key storage using system keyring
- Cross-platform support
- Active maintenance and security audits
- Simple API for secret key encryption

## 3. Configuration Storage

**Decision**: Use a single encrypted JSON configuration file:
- `~/.config/sex-cli/config.json` - Contains both organization data and encrypted tokens

**Rationale**:
- Follows XDG Base Directory specification
- Unified configuration storage
- JSON for human-readable format
- Encrypted sensitive data within the config
- Simpler configuration management

## 4. CLI Interface Design

**Decision**: Use clap for command-line parsing with a pattern similar to GitHub CLI.

**Rationale**:
- Type-safe command definitions
- Automatic help generation
- Familiar interface for developers
- Hierarchical command structure
- Easy to extend with new commands
- Support for both interactive and non-interactive modes

Example commands:
```
sex org list
sex org add <name> <slug>
sex issue list
sex issue view <id>
sex login <org> <token>
sex monitor <org> [project]
```

## 5. Testing Strategy

**Decision**: Implement multiple test types:
- Unit tests for core functionality
- Integration tests with mock data
- TUI component tests
- Command parsing tests

**Rationale**:
- Ensures reliability of core features
- Prevents regression issues
- Validates CLI user experience
- Tests encryption/decryption
- Verifies command parsing
- Allows testing without real Sentry credentials

## 6. Error Handling

**Decision**: Use Rust's Result type with anyhow for error handling.

**Rationale**:
- Type-safe error handling
- Context-aware error messages
- Clean error propagation
- Good for CLI application reliability
- Easy error conversion between types

## 7. TUI Implementation

**Decision**: Use crossterm for terminal manipulation.

**Rationale**:
- Cross-platform terminal support
- Raw mode for interactive UI
- Event handling for keyboard input
- Unicode box drawing support
- Color and styling capabilities 