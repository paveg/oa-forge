# oa-forge

Fast and correct OpenAPI → TypeScript code generator, built in Rust.

## Why oa-forge?

- **157x faster** than Orval (measured with [hyperfine](https://github.com/sharkdp/hyperfine))
- **Correct by default** — fixes known Orval bugs (`allOf` + required, circular `$ref`, discriminated unions)
- **Zero runtime dependency** — generated code uses plain `fetch`
- **Deterministic** — same input always produces identical output
- **Cleaner output** — separate files for types, client, hooks (not one giant file)

## Benchmarks

### End-to-end (process startup + parse + codegen + file I/O)

Measured with `hyperfine --warmup 2 --min-runs 5`:

| Spec | oa-forge | Orval v8.5 | Speedup |
|---|---|---|---|
| Petstore (4 endpoints) | **3.4 ms** | 536 ms | **157x** |
| Large (100+ endpoints) | **3.8 ms** | 623 ms | **165x** |

### In-process (criterion, no startup overhead)

| Benchmark | Time |
|---|---|
| Petstore full pipeline (parse → emit → format) | **133 µs** |
| Petstore parse only | 117 µs |
| Petstore IR convert | 5.8 µs |
| Petstore emit types | 1.2 µs |
| Petstore emit client | 2.5 µs |
| Petstore emit hooks | 3.1 µs |
| Large-scale full pipeline | **5.1 ms** |

> Run `cargo bench` to reproduce. Run `./scripts/bench-compare.sh` for the hyperfine comparison (requires `hyperfine` and `orval`).

## Quick Start

### 1. Install

```bash
npm install -D @oa-forge/cli
```

<details>
<summary>Other install methods</summary>

```bash
# Cargo
cargo install oa-forge

# From source
cargo install --path crates/cli
```

</details>

### 2. Generate

```bash
oa-forge generate --input openapi.yaml --output src/api --client fetch --hooks
```

This generates:

```
src/api/
├── types.gen.ts    # Pet, PetStatus, CreatePetBody, ...
├── client.gen.ts   # listPets(), createPet(), getPet(), ...
├── hooks.gen.ts    # useListPets(), useCreatePet(), ...
└── index.gen.ts    # Re-exports everything
```

Use it in your React component:

```tsx
import { useListPets, useCreatePet } from './api';

function PetList() {
  const { data: pets } = useListPets();
  const createPet = useCreatePet();

  return (
    <ul>
      {pets?.map(pet => <li key={pet.id}>{pet.name}</li>)}
      <button onClick={() => createPet.mutate({ name: 'Rex' })}>
        Add Pet
      </button>
    </ul>
  );
}
```

### 3. Configure (optional)

Create `oa-forge.toml` for project-wide settings:

```toml
input = "./openapi.yaml"
output = "./src/api"
client = "fetch"      # "fetch" | "axios" | "hono" | "angular"
hooks = true
zod = true            # Generate Zod schemas
split = "single"      # "single" | "tag" | "endpoint"
```

### More examples

```bash
# Types only (no client, no hooks)
oa-forge generate --input openapi.yaml --output src/api

# Axios client instead of fetch
oa-forge generate --input openapi.yaml --output src/api --client axios --hooks

# Zod + Valibot validation schemas
oa-forge generate --input openapi.yaml --output src/api --zod --valibot

# MSW mock handlers + faker data (for testing)
oa-forge generate --input openapi.yaml --output src/api --msw --mock

# Watch mode — regenerate on spec changes
oa-forge generate --input openapi.yaml --output src/api --hooks --watch

# Split output by tag (one directory per API tag)
oa-forge generate --input openapi.yaml --output src/api --hooks --split tag
```

## Generated Files

| File | Contents |
|---|---|
| `types.gen.ts` | Interfaces, enums, path/query params, request/response types |
| `client.gen.ts` | Typed fetch/axios client functions |
| `hooks.gen.ts` | TanStack Query hooks (useQuery, useMutation, useInfiniteQuery) |
| `zod.gen.ts` | Zod validation schemas |
| `valibot.gen.ts` | Valibot validation schemas |
| `msw.gen.ts` | MSW v2 request handlers |
| `mock.gen.ts` | Faker-based mock data factories |
| `index.gen.ts` | Barrel re-exports |

## Features

### Emitters

- **TypeScript types** — interfaces, enums, branded types, JSDoc
- **Fetch client** — typed wrappers with interceptors, AbortController, custom HTTP client injection
- **Axios client** — `setAxiosInstance()` pattern with `AxiosRequestConfig`
- **TanStack Query v5** — React, Vue, Solid, and Svelte Query variants
- **Hono RPC types** — `AppType` for end-to-end type safety
- **Angular HttpClient** — `@Injectable` service with `Observable<T>`
- **Zod schemas** — `z.lazy()` for circular refs, string/number constraints
- **Valibot schemas** — tree-shakeable alternative to Zod
- **MSW v2 handlers** — factory pattern with override support
- **Mock data** — description-aware faker generation

### Parser

- OpenAPI 3.0 / 3.1 (YAML and JSON)
- Swagger 2.0 (auto-converted to 3.0)
- `$ref` resolution: local, cross-file, double-indirect
- Circular reference detection (DFS)

### CLI

- Config: TOML, JSON, TypeScript
- Per-endpoint overrides (skip, rename operationId)
- Watch mode with debounce
- Split modes: single file, by tag, by endpoint
- Incremental generation (content hash skip)
- Dry-run mode

## Generated Code Comparison

From the same `petstore.yaml`:

| | oa-forge | Orval v8.5 |
|---|---|---|
| **Files** | 4 (types, client, hooks, index) | 1 (everything mixed) |
| **Lines** | ~200 | ~350 |
| **Separation of concerns** | Each file has a single responsibility | Types, client, hooks interleaved |
| **useQuery** | `useQuery` + `useSuspenseQuery` | `useQuery` only |
| **queryOptions** | TanStack Query v5 `queryOptions()` helper | Not generated |
| **prefetchQuery** | Generated for SSR / React Router | Not generated |
| **Loader support** | `ensureQueryData` loader function | Not generated |
| **Error handling** | Typed `ApiError<T>` class | Raw response (no error type) |
| **Interceptors** | `onRequest` / `onResponse` chains | Not available |
| **queryKey** | operationId-based (`['listPets']`) | Path-based (`['/pets']`) |
| **Readability** | Clean, minimal generics | Heavy `Awaited<ReturnType<typeof ...>>` nesting |

## Orval Migration

See [docs/orval-migration.md](docs/orval-migration.md) for a detailed migration guide.

### Key Orval bugs fixed

| Issue | Problem | oa-forge |
|---|---|---|
| [#1570](https://github.com/anymaniax/orval/issues/1570) | `allOf` + `required` not propagated | Correct merge |
| [#764](https://github.com/anymaniax/orval/issues/764) | Circular `$ref` crash | DFS detection, lazy refs |
| [#1935](https://github.com/anymaniax/orval/issues/1935) | Cross-file `$ref` failure | Recursive resolution |
| [#2535](https://github.com/anymaniax/orval/issues/2535) | Zod inline expansion | Reference-based generation |
| [#1316](https://github.com/anymaniax/orval/issues/1316) | Discriminated unions broken | Proper `discriminator` mapping |
| [#1526](https://github.com/anymaniax/orval/issues/1526) | `oneOf` inside `allOf` | Intersection types: `Base & (A \| B)` |

## Development

```bash
# Run tests
cargo test --all

# Run benchmarks
cargo bench

# Clippy
cargo clippy --all-targets --all-features

# Format
cargo fmt --all
```

## Architecture

```
crates/
├── parser          # OpenAPI YAML/JSON → internal OpenAPI types
├── ir              # OpenAPI types → intermediate representation (ApiSpec)
├── emitter-types   # IR → TypeScript type definitions
├── emitter-client  # IR → fetch client
├── emitter-query   # IR → TanStack Query hooks
├── emitter-zod     # IR → Zod schemas
├── emitter-valibot # IR → Valibot schemas
├── emitter-msw     # IR → MSW v2 handlers
├── emitter-mock    # IR → faker mock data
├── emitter-axios   # IR → Axios client
├── emitter-hono    # IR → Hono RPC types
├── emitter-angular # IR → Angular HttpClient service
├── formatter       # Post-processing (blank lines, import sorting)
└── cli             # CLI entry point (clap, rayon, notify)
```

## License

MIT
