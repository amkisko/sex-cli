# Development Guidelines

## Code Style

- Follow the official Rust style guide (rustfmt)
- Use meaningful variable and function names
- Keep functions small and focused
- Document public APIs with rustdoc
- Use Result for error handling
- Implement common traits (Debug, Clone, etc.) where appropriate

## Project Structure

```
src/
├── main.rs           # Entry point
├── commands.rs       # CLI command definitions
├── config.rs         # Configuration management
├── sentry.rs         # Sentry API client
├── tui.rs           # Terminal UI components
├── issue_viewer.rs   # Issue viewer component
└── dashboard.rs      # Real-time monitoring dashboard
```

## Testing

### Unit Tests

- Write tests in the same file as the code they test
- Use descriptive test names
- Test error cases
- Mock external dependencies
- Use test utilities from assert_fs and predicates

Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_save_and_load() -> Result<()> {
        // Setup
        let mut config = Config::default();
        config.add_organization("test".to_string(), "test-slug".to_string());
        
        // Test
        config.save()?;
        
        // Verify
        let loaded = Config::load()?;
        assert_eq!(config, loaded);
        Ok(())
    }
}
```

### Integration Tests

- Test complete features
- Use temporary files for testing
- Mock external services
- Verify error handling
- Test encryption/decryption

### TUI Tests

- Test component dimensions
- Verify rendering functions
- Test user input handling
- Check scrolling behavior
- Test box drawing

## Git Workflow

1. Create feature branch
2. Write tests
3. Implement feature
4. Run tests locally (`cargo test`)
5. Format code (`cargo fmt`)
6. Check lints (`cargo clippy`)
7. Create pull request
8. Wait for CI
9. Merge after approval

## Documentation

- Keep README.md updated
- Document new features
- Update architecture decisions
- Add examples for new commands
- Use rustdoc for API documentation

## Dependencies

Core dependencies:
- clap: Command line argument parsing
- anyhow: Error handling
- serde: Serialization/deserialization
- reqwest: HTTP client
- crossterm: Terminal manipulation
- sodiumoxide: Encryption
- keyring: Secure key storage
- dirs: XDG directory handling

Dev dependencies:
- assert_fs: Filesystem testing
- predicates: Test assertions
- tempfile: Temporary file handling

## Security

- Never commit tokens or credentials
- Use sodiumoxide for all crypto operations
- Store encryption keys in system keyring
- Validate all user input
- Handle sensitive data carefully
- Clear memory containing secrets
- Use secure random number generation

## Release Process

1. Update version in Cargo.toml
2. Update changelog
3. Run full test suite (`cargo test`)
4. Check formatting (`cargo fmt`)
5. Run lints (`cargo clippy`)
6. Create release tag
7. Build optimized binaries (`cargo build --release`)
8. Publish release 