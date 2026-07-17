# Contributing to tdh-protocol

Thank you for your interest in contributing to tdh-protocol! This document provides guidelines for contributing.

## Development Workflow

### 1. Clone and Setup

```bash
git clone <repository-url>
cd tdh-protocol

# Install Buf (if not already installed)
brew install bufbuild/buf/buf
```

### 2. Make Changes

Edit proto files in the `protos/` directory:
- Common types: `protos/common/`
- Domain types: `protos/onboarding/`, `protos/appointment/`, `protos/notification/`

### 3. Generate Code

```bash
# Generate code for all languages
make gen

# Or use buf directly
buf generate
```

### 4. Test Changes

```bash
# Run all tests
make test-all

# Test specific language
make test-rust
make test-scala
make test-python
make test-typescript
```

### 5. Verify Quality

```bash
# Lint proto files
make lint

# Check for breaking changes
make break
```

## Proto File Guidelines

### Naming Conventions

- **Files**: `lowercase_with_underscores.proto`
- **Packages**: `tdh.protocol.<domain>`
- **Messages**: `PascalCase`
- **Fields**: `snake_case`
- **Enums**: `PascalCase` with values in `SCREAMING_SNAKE_CASE`

### Adding New Messages

1. Define message in appropriate proto file
2. Use `oneof` for discriminated unions
3. Add corresponding test cases
4. Update this file's changelog

### Backward Compatibility

✅ **Safe Changes:**
- Add new field
- Add new message
- Add new enum value
- Add new oneof variant

❌ **Breaking Changes:**
- Remove field
- Change field number
- Change field type
- Remove enum value
- Rename package/message

## Submitting Changes

1. **Fork** the repository
2. **Create branch**: `git checkout -b feature/my-feature`
3. **Make changes** following guidelines
4. **Test**: `make test-all && make lint && make break`
5. **Commit**: `git commit -am "Add my feature"`
6. **Push**: `git push origin feature/my-feature`
7. **Open PR** with description

## Code Review

All changes require:
- At least one approval
- All tests passing
- No lint errors
- No breaking changes (unless major version bump)

## Versioning

This project follows semantic versioning:
- **Major**: Breaking changes
- **Minor**: New features, backward compatible
- **Patch**: Bug fixes, no proto changes
