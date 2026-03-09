// Type definitions for oa-forge configuration.
// Usage in oa-forge.config.ts:
//
//   import { defineConfig } from 'oa-forge/config';
//   export default defineConfig({ input: './petstore.yaml' });

export interface EndpointOverride {
  /** Custom operationId (replaces the spec's operationId). */
  operation_id?: string;
  /** Skip code generation for this endpoint. */
  skip?: boolean;
}

export interface OaForgeConfig {
  /** Path to the OpenAPI spec file (YAML or JSON). */
  input?: string;
  /** Output directory for generated files. */
  output?: string;
  /** HTTP client to generate. */
  client?: 'fetch';
  /** Generate TanStack Query hooks. */
  hooks?: boolean;
  /** Generate Zod schemas. */
  zod?: boolean;
  /** Generate Valibot schemas. */
  valibot?: boolean;
  /** Output split mode. */
  split?: 'single' | 'tag' | 'endpoint';
  /** Query framework for hooks generation. */
  query_framework?: 'react' | 'vue' | 'solid' | 'svelte';
  /** Per-endpoint overrides keyed by "METHOD /path" (e.g., "GET /pets/{petId}"). */
  overrides?: Record<string, EndpointOverride>;
}

/**
 * Define an oa-forge configuration with type checking.
 * This is an identity function that provides type safety for your config.
 */
export function defineConfig(config: OaForgeConfig): OaForgeConfig;
