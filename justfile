default:
    @just --list

# Build a plugin (default: jira-claude)
build plugin="jira-claude":
    cd {{ plugin }} && cargo build --release

# Run tests for a plugin
test plugin="jira-claude":
    cd {{ plugin }} && cargo test

# List all plugins
list:
    @ls -d */ 2>/dev/null | grep -v target | sed 's/\/$//' | grep -v '^\.'

# Show plugin info
info plugin="jira-claude":
    #!/usr/bin/env bash
    set -euo pipefail

    PLUGIN="{{ plugin }}"
    CARGO_FILE="$PLUGIN/Cargo.toml"
    MANIFEST_FILE="$PLUGIN/plugin.toml"

    if [ ! -f "$CARGO_FILE" ]; then
        echo "Plugin '$PLUGIN' not found"
        exit 1
    fi

    VERSION=$(grep '^version' "$CARGO_FILE" | head -1 | sed 's/.*"\(.*\)"/\1/')
    NAME=$(grep '^name' "$CARGO_FILE" | head -1 | sed 's/.*"\(.*\)"/\1/')
    DESC=$(grep '^description' "$CARGO_FILE" | head -1 | sed 's/.*"\(.*\)"/\1/')

    echo "Plugin: $NAME"
    echo "Version: $VERSION"
    echo "Description: $DESC"
    echo ""
    echo "Tag format: $PLUGIN-v$VERSION"

# Bump patch version for a plugin (0.1.0 → 0.1.1)
release-patch plugin="jira-claude" msg="": (_release plugin "patch" msg)

# Bump minor version for a plugin (0.1.0 → 0.2.0)
release-minor plugin="jira-claude" msg="": (_release plugin "minor" msg)

# Bump major version for a plugin (0.1.0 → 1.0.0)
release-major plugin="jira-claude" msg="": (_release plugin "major" msg)

_release plugin bump msg="":
    #!/usr/bin/env bash
    set -euo pipefail

    PLUGIN="{{ plugin }}"
    CARGO_FILE="$PLUGIN/Cargo.toml"
    MANIFEST_FILE="$PLUGIN/plugin.toml"

    if [ ! -f "$CARGO_FILE" ]; then
        echo "Plugin '$PLUGIN' not found"
        exit 1
    fi

    VERSION=$(grep '^version' "$CARGO_FILE" | head -1 | sed 's/.*"\(.*\)"/\1/')
    IFS='.' read -r MAJOR MINOR PATCH <<< "$VERSION"

    case "{{ bump }}" in
        patch) PATCH=$((PATCH + 1)) ;;
        minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
        major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    esac

    NEW_VERSION="$MAJOR.$MINOR.$PATCH"
    TAG="${PLUGIN}-v${NEW_VERSION}"

    # Update Cargo.toml
    sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$CARGO_FILE"
    echo "✓ $CARGO_FILE version: $VERSION → $NEW_VERSION"

    # Update plugin.toml if it exists
    if [ -f "$MANIFEST_FILE" ]; then
        sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$MANIFEST_FILE"
        echo "✓ $MANIFEST_FILE version: $NEW_VERSION"
    fi

    # Update marketplace.toml version for this plugin
    MARKETPLACE_FILE="marketplace.toml"
    if [ -f "$MARKETPLACE_FILE" ]; then
        # Find the plugin's [[plugins]] block and update its version
        # This uses awk to update only the version in the correct plugin block
        awk -v plugin="$PLUGIN" -v version="$NEW_VERSION" '
            /^\[\[plugins\]\]/ { in_block=1; current="" }
            in_block && /^name = / { gsub(/"/, "", $3); current=$3 }
            in_block && /^version = / && current==plugin { $0="version = \"" version "\"" }
            { print }
        ' "$MARKETPLACE_FILE" > "${MARKETPLACE_FILE}.tmp" && mv "${MARKETPLACE_FILE}.tmp" "$MARKETPLACE_FILE"
        echo "✓ $MARKETPLACE_FILE version: $NEW_VERSION"
    fi

    # Update Cargo.lock
    cd "$PLUGIN" && cargo check --quiet && cd ..
    echo "✓ Updated Cargo.lock"

    echo ""
    echo "Tag: $TAG"
    echo ""

    read -p "Create commit and tag? [Y/n] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        git add "$CARGO_FILE"
        if [ -f "$MANIFEST_FILE" ]; then
            git add "$MANIFEST_FILE"
        fi
        if [ -f "$MARKETPLACE_FILE" ]; then
            git add "$MARKETPLACE_FILE"
        fi
        git add "$PLUGIN/Cargo.lock"

        if [ -n "{{ msg }}" ]; then
            git commit -m "Release $TAG" -m "{{ msg }}"
        else
            git commit -m "Release $TAG"
        fi
        git tag "$TAG"
        echo "✓ Created commit and tag $TAG"

        read -p "Push to remote? (This triggers CI build) [Y/n] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Nn]$ ]]; then
            git push origin main
            git push origin "$TAG"
            echo "✓ Pushed commit and tag to origin"
            echo ""
            echo "CI will now build and release the plugin."
            echo "Check: https://github.com/grimurjonsson/to-tui-plugins/actions"
        fi
    fi

# Build plugin locally for testing
build-local plugin="jira-claude":
    #!/usr/bin/env bash
    set -euo pipefail

    PLUGIN="{{ plugin }}"

    echo "Building $PLUGIN..."
    cd "$PLUGIN" && cargo build --release

    echo ""
    echo "✓ Built successfully"
    echo ""
    echo "Library location:"

    if [[ "$OSTYPE" == "darwin"* ]]; then
        ls -la "target/release/lib${PLUGIN//-/_}.dylib" 2>/dev/null || true
    elif [[ "$OSTYPE" == "linux"* ]]; then
        ls -la "target/release/lib${PLUGIN//-/_}.so" 2>/dev/null || true
    fi

    echo ""
    echo "To test locally in to-tui:"
    echo "  totui plugin install ../to-tui-plugins/$PLUGIN"

# Check if plugin dependencies can use git references
check-deps plugin="jira-claude":
    #!/usr/bin/env bash
    set -euo pipefail

    PLUGIN="{{ plugin }}"
    CARGO_FILE="$PLUGIN/Cargo.toml"

    echo "Checking dependencies in $CARGO_FILE..."
    echo ""

    if grep -q 'path = ' "$CARGO_FILE"; then
        echo "⚠️  Found path dependencies (will not work in release):"
        grep 'path = ' "$CARGO_FILE"
        echo ""
        echo "For release, update to git references:"
        echo '  totui-plugin-interface = { git = "https://github.com/grimurjonsson/to-tui", branch = "main" }'
    else
        echo "✓ No path dependencies found"
    fi
