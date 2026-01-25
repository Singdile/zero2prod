# AGENTS.md

This file provides guidelines and commands for agentic coding agents operating in the `zero2prod` repository.

## 1. Build, Lint, and Test Commands

### Build
*   `cargo build`: Compiles the project.
*   `cargo build --release`: Compiles the project in release mode (optimized).

### Lint
*   `cargo clippy`: Lints the code using Clippy, a Rust linter.
*   `cargo fmt --check`: Checks if the code is formatted according to Rust style guidelines without modifying files.
*   `cargo fmt`: Formats the code according to Rust style guidelines.

### Test
*   `cargo test`: Runs all tests in the project.
*   `cargo test <test_name>`: Runs a specific test by name. For example, `cargo test subscribe_returns_a_200_for_valid_form_data`.
*   `cargo test -- --ignored`: Runs tests marked with `#[ignore]`.
*   `cargo test --doc`: Runs tests in documentation examples.
*   `cargo watch -x test`: Continuously runs tests on file changes (requires `cargo-watch` to be installed: `cargo install cargo-watch`).

## 2. Code Style Guidelines

### Imports
*   Organize `use` statements at the top of each module.
*   Prefer `use crate::module_name::item` for internal modules.
*   Group related imports.
*   Avoid glob imports (`use crate::module_name::*`) unless explicitly justified (e.g., for prelude modules).

### Formatting
*   Use `cargo fmt` for automatic formatting.
*   Adhere to Rust's official style guide (enforced by `cargo fmt`).

### Types
*   Rust is a statically and strongly typed language. Ensure all types are explicit or can be clearly inferred by the compiler.
*   Leverage Rust's ownership and borrowing system for memory safety.
*   Use `struct` for composite data types and `enum` for representing distinct variants.

### Naming Conventions
*   **Modules, Functions, and Variables:** `snake_case` (e.g., `my_function`, `my_variable`, `my_module`).
*   **Types (Structs, Enums, Traits):** `PascalCase` (e.g., `MyStruct`, `MyEnum`, `MyTrait`).
*   **Constants:** `SCREAMING_SNAKE_CASE` (e.g., `MY_CONSTANT`).

### Error Handling
*   Prefer `Result<T, E>` for recoverable errors and `Option<T>` for representing the presence or absence of a value.
*   Use the `?` operator for propagating `Result` and `Option` errors concisely.
*   Use `match` statements for exhaustive error handling and pattern matching.
*   Avoid `unwrap()`, `expect()`, `panic!()` in production-ready code unless absolutely necessary (e.g., for unrecoverable errors during application startup). In tests, `expect()` and `unwrap()` are acceptable.
*   Provide clear and informative error messages.

### Comments
*   Use `///` for documentation comments for public items.
*   Use `//` for inline comments to explain complex logic or provide context.
*   Avoid redundant comments that merely restate the code.

### Asynchronous Code
*   Use `async` and `await` for asynchronous operations.
*   Ensure proper use of `#[tokio::main]` or `actix_web::main` for entry points of async applications/tests.

### Database Interactions
*   Use `sqlx::query!` and `sqlx::query_as!` for type-safe SQL queries.
*   Handle database connection pooling carefully.

## 3. Cursor/Copilot Rules

No specific Cursor (.cursor/rules/ or .cursorrules) or Copilot (.github/copilot-instructions.md) rules were found in this repository.

## 4. commuication with user
*   **language limited**: using Chinese(simplify) to talk with me
*   **tech-term**: remain English when facing to specific English-term
