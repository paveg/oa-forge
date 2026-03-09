# Migrating from Orval to oa-forge

## Config File Mapping

| Orval (`orval.config.ts`)               | oa-forge (`oa-forge.toml`)         |
|------------------------------------------|------------------------------------|
| `input`                                  | `input`                            |
| `output.target`                          | `output`                           |
| `output.client = 'react-query'`          | `hooks = true`                     |
| `output.client = 'vue-query'`            | `hooks = true`, `query_framework = "vue"` |
| `output.client = 'svelte-query'`         | `hooks = true`, `query_framework = "svelte"` |
| `output.mode = 'single'`                 | `split = "single"` (default)       |
| `output.mode = 'tags'`                   | `split = "tag"`                    |
| `output.mode = 'tags-split'`             | `split = "tag"`                    |
| `output.mode = 'split'`                  | `split = "tag"` (closest match)    |

### Example

**Orval (`orval.config.ts`)**:
```typescript
export default {
  petstore: {
    input: './openapi.yaml',
    output: {
      target: './src/api',
      client: 'react-query',
      mode: 'tags',
    },
  },
};
```

**oa-forge (`oa-forge.toml`)**:
```toml
input = "./openapi.yaml"
output = "./src/api"
hooks = true
split = "tag"
```

## CLI Flag Mapping

| Orval CLI                          | oa-forge CLI                              |
|------------------------------------|-------------------------------------------|
| `orval --input spec.yaml`          | `oa-forge generate --input spec.yaml`     |
| `orval --output ./src/api`         | `oa-forge generate --output ./src/api`    |
| `orval --watch`                    | `oa-forge generate --watch`               |
| `orval --config orval.config.ts`   | `oa-forge --config oa-forge.toml`         |

## Generated File Comparison

| File Purpose          | Orval                   | oa-forge                |
|-----------------------|-------------------------|-------------------------|
| Type definitions      | `*.ts` (inline)         | `types.gen.ts`          |
| HTTP client functions | `*.ts` (inline)         | `client.gen.ts`         |
| Query hooks           | `*.ts` (inline)         | `hooks.gen.ts`          |
| Barrel export         | (custom)                | `index.gen.ts`          |

## Key Differences

1. **Performance**: oa-forge is 10-100x faster than Orval (Rust vs Node.js).
2. **Deterministic output**: Same input always produces the same output.
3. **Separate files**: Types, client, and hooks are always in separate files.
4. **No runtime dependency**: Generated client uses plain `fetch`, no library needed.
5. **Branded types**: ID-like types get branded automatically (e.g., `UserId`).

## Additional Config Mapping

| Orval                                    | oa-forge                                  |
|------------------------------------------|-------------------------------------------|
| `output.override.zod`                    | `zod = true`                              |
| `output.override.mock.type = 'msw'`     | `msw = true`                              |
| `output.mock = true`                     | `mock = true`                             |
| `output.client = 'axios'`               | `client = "axios"`                        |
| `output.client = 'angular'`             | `client = "angular"`                      |
| Per-operation overrides                  | `[overrides."METHOD /path"]`              |

## Features Not Yet Supported

- Custom transformer functions (use interceptors instead)

## Common Orval Issues Fixed in oa-forge

- **#1570**: `allOf` + `required` propagation — correctly merges required fields
- **#764**: Circular `$ref` — detected and emitted as lazy references
- **#1935**: Cross-file `$ref` — resolved recursively with double-indirect support
- **#2535**: Zod inline expansion — reference-based generation (Phase 2)
- **#1316**: Discriminated unions — proper `discriminator` support
- **#1526**: `oneOf` inside `allOf` — intersection types (`Base & (A | B)`)
- **#2710**: `anyOf` with nullable enum — correctly handled
- **#1077**: `additionalProperties` with `$ref` — `Record<string, T>` support
