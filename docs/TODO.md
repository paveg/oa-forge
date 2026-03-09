# oa-forge TODO

## Current Status

All phases complete. 14 crates, 111 tests passing, crates.io publish-ready.
Rust 1.93.1, edition 2024.

---

## Phase 1: MVP

### Parser / $ref Resolver

- [x] OpenAPI 3.0/3.1 YAML/JSON parsing (serde)
- [x] Basic `$ref` resolution (`#/components/schemas/X`)
- [x] Circular reference detection (DFS + visited set)
- [x] Cross-file `$ref` resolution (relative path: `./models.yaml#/components/schemas/X`)
- [x] Double-indirect `$ref` (A -> index.yaml -> B.yaml) — Orval #1935
- [x] `$ref` resolution cache (thread-local `HashMap<String, TypeRepr>`)
- [x] `$ref` in `parameters`, `requestBodies`, `responses`, `headers`
- [x] `additionalProperties` with `$ref` — Orval #1077
- [x] OpenAPI 3.1 JSON Schema alignment (`type` as array, `$defs`, `prefixItems`)

### IR / Schema Conversion

- [x] Primitive types (string, number, integer, boolean)
- [x] Object with properties and required
- [x] Array with items
- [x] Enum (string, integer)
- [x] Nullable
- [x] `$ref` as named reference (lazy, not eagerly dereferenced)
- [x] allOf merge with required propagation — fixes Orval #1570
- [x] oneOf / anyOf -> Union type
- [x] discriminator resolution -> TypeScript discriminated union — Orval #1316
- [x] oneOf nested inside allOf — Orval #1526 (→ Intersection type: `Base & (A | B)`)
- [x] anyOf with nullable enum — Orval #2710
- [x] `additionalProperties` support (`Record<string, T>`)
- [x] `default` value preservation
- [x] Multiple response status codes (any 2xx with JSON content)
- [x] Non-JSON content types (multipart/form-data, text/plain, application/octet-stream)
- [x] Header/Cookie parameters in IR (ParamLocation enum already supported)
- [x] `description` propagation to IR (for JSDoc generation)

### Emitter: TypeScript Types

- [x] Interface generation (Object schemas)
- [x] Type alias generation (Union, Enum, Primitive)
- [x] Endpoint PathParams / QueryParams interfaces
- [x] Endpoint Response / Body type aliases
- [x] JSDoc comments from schema descriptions
- [x] `readonly` modifier for readOnly properties
- [x] `additionalProperties` -> `Record<string, T>` / index signature
- [x] Generic `Partial<T>` / `Required<T>` for PATCH bodies
- [x] Branded types support — Orval #1222
- [x] Namespace / barrel export organization (index.gen.ts barrel file)

### Emitter: Fetch Client

- [x] Typed fetch wrapper with RequestConfig
- [x] Path parameter interpolation (`/pets/${pathParams.petId}`)
- [x] Query parameter serialization (buildQuery helper)
- [x] Content-Type header for JSON bodies
- [x] Import types from types.gen.ts
- [x] Non-JSON response handling (blob, text, void for 204)
- [x] Error response typing (error response schema in IR)
- [x] Request/response interceptors
- [x] AbortController / timeout support
- [x] Array query parameter serialization (comma, multi, brackets)
- [x] Custom HTTP client injection (mutator pattern)

### Emitter: TanStack Query v5 Hooks

- [x] useQuery / useSuspenseQuery for GET endpoints
- [x] useMutation for POST/PUT/PATCH/DELETE endpoints
- [x] queryKey with operationId — Orval #2096
- [x] queryOptions helper — Orval #1679
- [x] Proper path/query params separation in hooks
- [x] Mutation variables typed correctly (pathParams + body)
- [x] Import from types.gen.ts and client.gen.ts
- [x] `enabled` option pattern (skip query when params undefined)
- [x] `useInfiniteQuery` for paginated endpoints
- [x] `prefetchQuery` helpers
- [x] Vue Query / Solid Query / Svelte Query variants
- [x] queryClient injection — Orval #1278
- [x] React Query loader function support — Orval #2024

### Formatter

- [x] Collapse multiple blank lines
- [x] Consistent indentation (2-space, tab normalization)
- [x] Trailing comma consistency (emitters produce consistent output)
- [x] Semicolon consistency (emitters produce consistent output)
- [x] Import sorting (type imports first, then value imports, alphabetical)

### CLI

- [x] `oa-forge generate` command with clap
- [x] `--input`, `--output`, `--client`, `--hooks` flags
- [x] Config file support (oa-forge.toml / oa-forge.config.ts)
- [x] `--watch` mode (notify crate)
- [x] Output mode: single file / split by tag (split by endpoint deferred to Phase 2)
- [x] Validation ON/OFF flag
- [x] `--dry-run` flag (preview without writing)
- [x] Orval config migration guide / compatibility layer
- [x] Error reporting with spec location (line:col via serde_yaml)

### Testing

- [x] Parser unit tests (4 passing)
- [x] Test fixtures: petstore.yaml, allof-required.yaml, circular-ref.yaml
- [x] Snapshot tests with `insta` crate (types: 9, client: 3, query: 3)
- [x] oneOf + discriminator fixture (oneof-discriminator.yaml)
- [x] Cross-file $ref fixture (cross-file/)
- [x] Large-scale spec fixture (100+ endpoints)
- [x] TypeScript compilation check (`tsc --noEmit` on generated code)
- [x] Integration test: end-to-end pipeline assertion (54 tests)
- [x] Edge case: empty spec, spec with no paths, spec with no schemas

### Benchmarks

- [x] `benches/codegen.rs` with criterion
- [x] petstore.yaml benchmark (~83µs full pipeline)
- [x] Medium spec (50 endpoints) benchmark
- [x] Large spec (200+ endpoints, MS Graph equivalent) benchmark
- [x] Comparison script: hyperfine vs Orval, hey-api, openapi-typescript
- [x] CI regression check (10%+ slowdown = warning)

### Performance

- [x] Rayon parallel emitter execution (rayon::join for 3 emitters)
- [x] Parallel file I/O (par_iter for writing)
- [x] $ref resolution cache
- [x] Incremental generation (content hash skip when unchanged)

---

## Phase 2: Differentiation

### Zod Schema Emitter (`emitter-zod` crate)

- [x] Reference-based generation (no inline expansion) — fixes Orval #2535
- [x] Circular reference -> `z.lazy()` — fixes Orval #2332
- [x] allOf/oneOf/anyOf in requestBody — Orval #1327
- [x] Enum -> `z.enum()`
- [x] Nullable -> `z.nullable()`
- [x] Optional -> `z.optional()`
- [x] Default values -> `z.default()`
- [x] String constraints (minLength, maxLength, pattern, format) -> `z.string().min().max().regex()`
- [x] Number constraints (minimum, maximum, multipleOf)
- [x] Array constraints (minItems, maxItems)

### Valibot Schema Emitter (`emitter-valibot` crate)

- [x] Valibot equivalent of Zod emitter (tree-shakeable, smaller bundle)

### Watch Mode

- [x] `notify` crate file watcher
- [x] Hash comparison (regenerate only changed specs)
- [x] Debounce (avoid rapid re-generation)

### Config File

- [x] `oa-forge.toml` support
- [x] TypeScript config (oa-forge.config.ts) for type-safe configuration
- [x] Per-endpoint overrides (custom operationId, skip, transform)

### Output Modes

- [x] Single file mode (default, current)
- [x] Split by tag (one file per tag)
- [x] Split by endpoint (one file per operation)

---

## Phase 3: Ecosystem

### MSW Handler Generation

- [x] MSW v2 request handler generation
- [x] Faker-based mock data (independent from MSW) — Orval #1832
- [x] MSW handler override support — Orval #1206

### Swagger 2.0 Support

- [x] Swagger 2.0 -> OpenAPI 3.0 conversion layer

### npm Distribution

- [x] `@oa-forge/cli` meta-package (postinstall binary selection)
- [x] `@oa-forge/cli-darwin-arm64`
- [x] `@oa-forge/cli-darwin-x64`
- [x] `@oa-forge/cli-linux-x64`
- [x] `@oa-forge/cli-linux-arm64`
- [x] `@oa-forge/cli-win32-x64`
- [x] GitHub Actions: cross-compile + publish workflow

### Additional Clients

- [x] Axios client emitter
- [x] Hono RPC type emitter
- [x] Angular HttpClient emitter

### CI / Quality

- [x] GitHub Actions: build + test on PR
- [x] GitHub Actions: benchmark on PR (comment with comparison)
- [x] Clippy + rustfmt check
- [x] MSRV policy
- [x] CHANGELOG.md automation
- [x] crates.io publish workflow

---

## Release Checklist (MVP)

- [x] All P0 features implemented
- [x] petstore.yaml: 10x+ faster than Orval (benchmark proof)
- [x] Large spec: seconds, not minutes (benchmark proof)
- [x] allOf + required: correct (Orval #1570 reproduction test)
- [x] Circular ref: no crash (Orval #764 reproduction test)
- [x] TanStack Query v5 hooks: production-usable
- [x] README with benchmarks, quick start, migration guide
- [x] `tsc --noEmit` passes on all generated output
- [ ] Published to crates.io
- [ ] Published to npm (@oa-forge/cli)
- [ ] Show HN / r/rust / r/typescript / r/reactjs posts

---

## References

- [Research: Strategy](https://github.com/paveg/research/blob/main/docs/openapi-codegen-ecosystem/strategy.md)
- [Research: Roadmap](https://github.com/paveg/research/blob/main/docs/openapi-codegen-ecosystem/roadmap.md)
- [Research: Implementation Guide](https://github.com/paveg/research/blob/main/docs/openapi-codegen-ecosystem/implementation-guide.md)
- [Research: Pain Points Deep Dive](https://github.com/paveg/research/blob/main/docs/openapi-codegen-ecosystem/pain-points-deep-dive.md)
- Orval Issues: #1570 #764 #1935 #2535 #2332 #1316 #1526 #2710 #1077 #1327 #2096 #1679 #1278 #2024 #1222 #1832 #1206
