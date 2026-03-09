# oa-forge TODO

## Current Status

Initial scaffolding complete. 7 crates, petstore.yaml end-to-end working.
Rust 1.93.1, edition 2024.

---

## Phase 1: MVP

### Parser / $ref Resolver

- [x] OpenAPI 3.0/3.1 YAML/JSON parsing (serde)
- [x] Basic `$ref` resolution (`#/components/schemas/X`)
- [x] Circular reference detection (DFS + visited set)
- [ ] Cross-file `$ref` resolution (relative path: `./models.yaml#/components/schemas/X`)
- [ ] Double-indirect `$ref` (A -> index.yaml -> B.yaml) — Orval #1935
- [ ] `$ref` resolution cache (`HashMap<RefPath, Arc<TypeRepr>>`)
- [ ] `$ref` in `parameters`, `requestBodies`, `responses`, `headers`
- [ ] `additionalProperties` with `$ref` — Orval #1077
- [ ] OpenAPI 3.1 JSON Schema alignment (`type` as array, `$defs`, `prefixItems`)

### IR / Schema Conversion

- [x] Primitive types (string, number, integer, boolean)
- [x] Object with properties and required
- [x] Array with items
- [x] Enum (string, integer)
- [x] Nullable
- [x] `$ref` as named reference (lazy, not eagerly dereferenced)
- [x] allOf merge with required propagation — fixes Orval #1570
- [x] oneOf / anyOf -> Union type
- [ ] discriminator resolution -> TypeScript discriminated union — Orval #1316
- [ ] oneOf nested inside allOf — Orval #1526
- [ ] anyOf with nullable enum — Orval #2710
- [ ] `additionalProperties` support (`Record<string, T>`)
- [ ] `default` value preservation
- [ ] Multiple response status codes (currently only 200/201)
- [ ] Non-JSON content types (multipart/form-data, text/plain, application/octet-stream)
- [ ] Header/Cookie parameters in IR
- [ ] `description` propagation to IR (for JSDoc generation)

### Emitter: TypeScript Types

- [x] Interface generation (Object schemas)
- [x] Type alias generation (Union, Enum, Primitive)
- [x] Endpoint PathParams / QueryParams interfaces
- [x] Endpoint Response / Body type aliases
- [ ] JSDoc comments from schema descriptions
- [ ] `readonly` modifier for readOnly properties
- [ ] `additionalProperties` -> `Record<string, T>` / index signature
- [ ] Generic `Partial<T>` / `Required<T>` for PATCH bodies
- [ ] Branded types support — Orval #1222
- [ ] Namespace / barrel export organization

### Emitter: Fetch Client

- [x] Typed fetch wrapper with RequestConfig
- [x] Path parameter interpolation (`/pets/${pathParams.petId}`)
- [x] Query parameter serialization (buildQuery helper)
- [x] Content-Type header for JSON bodies
- [x] Import types from types.gen.ts
- [ ] Non-JSON response handling (blob, text, void for 204)
- [ ] Error response typing
- [ ] Request/response interceptors
- [ ] AbortController / timeout support
- [ ] Array query parameter serialization (comma, multi, brackets)
- [ ] Custom HTTP client injection (mutator pattern)

### Emitter: TanStack Query v5 Hooks

- [x] useQuery / useSuspenseQuery for GET endpoints
- [x] useMutation for POST/PUT/PATCH/DELETE endpoints
- [x] queryKey with operationId — Orval #2096
- [x] queryOptions helper — Orval #1679
- [x] Proper path/query params separation in hooks
- [x] Mutation variables typed correctly (pathParams + body)
- [x] Import from types.gen.ts and client.gen.ts
- [ ] `enabled` option pattern (skip query when params undefined)
- [ ] `useInfiniteQuery` for paginated endpoints
- [ ] `prefetchQuery` helpers
- [ ] Vue Query / Solid Query / Svelte Query variants
- [ ] queryClient injection — Orval #1278
- [ ] React Query loader function support — Orval #2024

### Formatter

- [x] Collapse multiple blank lines
- [ ] Consistent indentation (2-space)
- [ ] Trailing comma consistency
- [ ] Semicolon consistency
- [ ] Import sorting

### CLI

- [x] `oa-forge generate` command with clap
- [x] `--input`, `--output`, `--client`, `--hooks` flags
- [ ] Config file support (oa-forge.toml / oa-forge.config.ts)
- [ ] `--watch` mode (notify crate) — flag parsed but not implemented
- [ ] Output mode: single file / split by tag / split by endpoint
- [ ] Validation ON/OFF flag
- [ ] `--dry-run` flag (preview without writing)
- [ ] Orval config migration guide / compatibility layer
- [ ] Error reporting with spec location (line:col)

### Testing

- [x] Parser unit tests (4 passing)
- [x] Test fixtures: petstore.yaml, allof-required.yaml, circular-ref.yaml
- [ ] Snapshot tests with `insta` crate
- [ ] oneOf + discriminator fixture (oneof-discriminator.yaml)
- [ ] Cross-file $ref fixture (cross-file/)
- [ ] Large-scale spec fixture (100+ endpoints)
- [ ] TypeScript compilation check (`tsc --noEmit` on generated code)
- [ ] Integration test: end-to-end pipeline assertion
- [ ] Edge case: empty spec, spec with no paths, spec with no schemas

### Benchmarks

- [ ] `benches/codegen.rs` with criterion
- [ ] petstore.yaml benchmark
- [ ] Medium spec (50 endpoints) benchmark
- [ ] Large spec (200+ endpoints, MS Graph equivalent) benchmark
- [ ] Comparison script: hyperfine vs Orval, hey-api, openapi-typescript
- [ ] CI regression check (10%+ slowdown = warning)

### Performance

- [ ] Rayon parallel emitter execution
- [ ] Parallel file I/O
- [ ] $ref resolution cache
- [ ] Incremental generation (spec hash cache, changed schema only)

---

## Phase 2: Differentiation

### Zod Schema Emitter (`emitter-zod` crate)

- [ ] Reference-based generation (no inline expansion) — fixes Orval #2535
- [ ] Circular reference -> `z.lazy()` — fixes Orval #2332
- [ ] allOf/oneOf/anyOf in requestBody — Orval #1327
- [ ] Enum -> `z.enum()`
- [ ] Nullable -> `z.nullable()`
- [ ] Optional -> `z.optional()`
- [ ] Default values -> `z.default()`
- [ ] String constraints (minLength, maxLength, pattern, format) -> `z.string().min().max().regex()`
- [ ] Number constraints (minimum, maximum, multipleOf)
- [ ] Array constraints (minItems, maxItems)

### Valibot Schema Emitter

- [ ] Valibot equivalent of Zod emitter (tree-shakeable, smaller bundle)

### Watch Mode

- [ ] `notify` crate file watcher
- [ ] Hash comparison (regenerate only changed specs)
- [ ] Debounce (avoid rapid re-generation)

### Config File

- [ ] `oa-forge.toml` support
- [ ] TypeScript config (oa-forge.config.ts) for type-safe configuration
- [ ] Per-endpoint overrides (custom operationId, skip, transform)

### Output Modes

- [ ] Single file mode (default, current)
- [ ] Split by tag (one file per tag)
- [ ] Split by endpoint (one file per operation)

---

## Phase 3: Ecosystem

### MSW Handler Generation

- [ ] MSW v2 request handler generation
- [ ] Faker-based mock data (independent from MSW) — Orval #1832
- [ ] MSW handler override support — Orval #1206

### Swagger 2.0 Support

- [ ] Swagger 2.0 -> OpenAPI 3.0 conversion layer

### npm Distribution

- [ ] `@oa-forge/cli` meta-package (postinstall binary selection)
- [ ] `@oa-forge/cli-darwin-arm64`
- [ ] `@oa-forge/cli-darwin-x64`
- [ ] `@oa-forge/cli-linux-x64`
- [ ] `@oa-forge/cli-linux-arm64`
- [ ] `@oa-forge/cli-win32-x64`
- [ ] GitHub Actions: cross-compile + publish workflow

### Additional Clients

- [ ] Axios client emitter
- [ ] Hono RPC type emitter
- [ ] Angular HttpClient emitter

### CI / Quality

- [ ] GitHub Actions: build + test on PR
- [ ] GitHub Actions: benchmark on PR (comment with comparison)
- [ ] Clippy + rustfmt check
- [ ] MSRV policy
- [ ] CHANGELOG.md automation
- [ ] crates.io publish workflow

---

## Release Checklist (MVP)

- [ ] All P0 features implemented
- [ ] petstore.yaml: 10x+ faster than Orval (benchmark proof)
- [ ] Large spec: seconds, not minutes (benchmark proof)
- [ ] allOf + required: correct (Orval #1570 reproduction test)
- [ ] Circular ref: no crash (Orval #764 reproduction test)
- [ ] TanStack Query v5 hooks: production-usable
- [ ] README with benchmarks, quick start, migration guide
- [ ] `tsc --noEmit` passes on all generated output
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
