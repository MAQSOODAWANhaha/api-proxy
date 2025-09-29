## Project Overview

This is an enterprise-level AI service proxy platform named "AI Proxy Platform". It's built with Rust and the Pingora framework, featuring a dual-port separation architecture. The platform acts as a unified gateway for various AI services like OpenAI, Google Gemini, and Anthropic Claude.

**Key Technologies:**

*   **Backend:** Rust, Pingora, Axum, Sea-ORM, Tokio
*   **Database:** SQLite
*   **Cache:** Redis
*   **Frontend:** Vue 3, TypeScript, Vite, Element Plus

**Architecture:**

The project uses a dual-port architecture:
*   **Pingora Proxy (Port 8080):** Handles high-performance AI request proxying, load balancing, and authentication.
*   **Axum Management (Port 9090):** Provides a web interface and APIs for user management, API key configuration, statistics, and system settings.

Both services share a common data layer consisting of a SQLite database and a Redis cache.

## Building and Running

**Prerequisites:**

*   Rust toolchain (version 1.75+)
*   Redis (optional, for caching)

**Installation and Startup:**

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/MAQSOODAWANhaha/api-proxy.git
    cd api-proxy
    ```

3.  **Start the services (dual-port mode):**
    ```bash
    cargo run --bin api-proxy
    ```

    *   The Pingora proxy service will be available at `http://localhost:8080`.
    *   The Axum management service will be available at `http://localhost:9090`.

**Running the Frontend (Development):**

1.  **Navigate to the frontend directory:**
    ```bash
    cd frontend
    ```

2.  **Install dependencies:**
    ```bash
    npm install
    ```

3.  **Start the development server:**
    ```bash
    npm run dev
    ```

## Development Conventions

*   **Code Formatting:** Use `cargo fmt` for Rust and `npm run lint` for the frontend to ensure consistent code style.
*   **Code Quality:** Use `cargo clippy` to check for common mistakes and style issues in the Rust code.
*   **Testing:**
    *   Run backend tests with `cargo test`.
    *   The frontend testing setup is not fully detailed, but `vue-tsc` is used for type checking.
*   **Database Migrations:** Database schema changes are managed through the `migration` crate. New migrations should be created as new files in the `migration/src/` directory.
*   **Documentation:** The `docs` directory contains detailed design and API documentation. It's important to keep these documents updated.
