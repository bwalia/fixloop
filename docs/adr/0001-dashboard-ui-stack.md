# ADR-0001: Dashboard UI stack — Leptos (CSR, embedded)

* Status: Proposed
* Date: 2026-11-14

## Context
The build brief prefers Leptos (Rust → WASM) served by the same binary. It asks for an ADR if a TypeScript frontend would be much better for contributors. Requirements: about 8 pages, mostly tables/forms/charts, live job logs, dark mode, keyboard shortcuts, every page backed by a public API, and a single static binary.

## Options
1. **Leptos CSR, embedded into the binary** (via `rust-embed`) and talking only to the public REST API.
2. Leptos SSR/hydration with server functions.
3. TypeScript SPA (React/Svelte + Vite), embedded the same way.

## Decision
Option 1, **Leptos 0.8.x stable (CSR)**. Pin it and do not adopt 0.9 until it is GA.

* It keeps the project single-language and one toolchain for contributors (cargo + trunk). This matters for a Rust-first CNCF project.
* Pure CSR against the public REST API is **forced dogfooding**: "every page has a matching API endpoint" holds by construction, and the UI can be replaced later without touching the server.
* We avoid server functions (option 2) because they would couple the UI to server internals and work against the API-first rule.

## Consequences
* Charts: use a small SVG chart component of our own, or `charming` if its license and maintenance check out. We avoid a heavy JS charting library.
* Frontend contributors who don't know Rust face a higher barrier. Mitigation: a strict API contract (OpenAPI), so a TS dashboard could live as a separate community project if demand appears.
* WASM bundle size is to be watched (target < 1.5 MB gzipped). Use `wasm-opt` in release.

## Revisit if
Leptos maintenance stalls, or contributor feedback shows the UI is a bottleneck.
