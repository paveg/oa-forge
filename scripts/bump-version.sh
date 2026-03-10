#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 0.2.0"
  exit 1
fi

VERSION="$1"

# Validate semver format
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
  echo "Error: '$VERSION' is not a valid semver version"
  exit 1
fi

echo "Bumping all packages to v${VERSION}..."

# 1. Rust workspace version (single source of truth for all 14 crates)
sed -i '' "s/^version = \"[^\"]*\"/version = \"${VERSION}\"/" Cargo.toml

# 2. Internal crate dependency versions in workspace
sed -i '' "s/version = \"[^\"]*\", path = \"crates\//version = \"${VERSION}\", path = \"crates\//g" Cargo.toml

# 3. npm packages (meta + 5 platform-specific)
for pkg in npm/cli npm/cli-darwin-arm64 npm/cli-darwin-x64 npm/cli-linux-x64 npm/cli-linux-arm64 npm/cli-win32-x64; do
  if [ -f "$pkg/package.json" ]; then
    # Update version field
    sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"${VERSION}\"/" "$pkg/package.json"
  fi
done

# 4. Update optionalDependencies versions in meta package
sed -i '' "s/\"@oa-forge\/cli-[^\"]*\": \"[^\"]*\"/&/g" npm/cli/package.json
for platform in darwin-arm64 darwin-x64 linux-x64 linux-arm64 win32-x64; do
  sed -i '' "s/\"@oa-forge\/cli-${platform}\": \"[^\"]*\"/\"@oa-forge\/cli-${platform}\": \"${VERSION}\"/" npm/cli/package.json
done

# 5. Verify changes
echo ""
echo "=== Rust workspace version ==="
grep '^version = ' Cargo.toml | head -1

echo ""
echo "=== npm package versions ==="
for pkg in npm/cli npm/cli-darwin-arm64 npm/cli-darwin-x64 npm/cli-linux-x64 npm/cli-linux-arm64 npm/cli-win32-x64; do
  echo -n "  $pkg: "
  grep '"version"' "$pkg/package.json" | head -1 | tr -d ' '
done

echo ""
echo "=== Internal crate dependency versions ==="
grep 'version = ".*", path = "crates/' Cargo.toml | head -3
echo "  ... ($(grep -c 'version = ".*", path = "crates/' Cargo.toml) total)"

echo ""
echo "Done! Next steps:"
echo "  git add -A"
echo "  git commit -m 'chore(release): v${VERSION}'"
echo "  git tag v${VERSION}"
echo "  git push origin main --tags"
