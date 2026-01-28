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

# Build and deploy plugin locally for testing in totui
test-deploy plugin:
    #!/usr/bin/env bash
    set -euo pipefail

    PLUGIN="{{ plugin }}"
    PLUGIN_DIR="$PLUGIN"

    # Determine plugins directory based on OS (matches totui's get_plugins_dir)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        PLUGINS_DEST="$HOME/Library/Application Support/to-tui/plugins/$PLUGIN"
    elif [[ "$OSTYPE" == "linux"* ]]; then
        PLUGINS_DEST="${XDG_DATA_HOME:-$HOME/.local/share}/to-tui/plugins/$PLUGIN"
    elif [[ "$OSTYPE" == "msys"* ]] || [[ "$OSTYPE" == "win32"* ]]; then
        PLUGINS_DEST="$LOCALAPPDATA/to-tui/plugins/$PLUGIN"
    else
        PLUGINS_DEST="$HOME/.local/share/to-tui/plugins/$PLUGIN"
    fi

    # Validate plugin exists
    if [ ! -d "$PLUGIN_DIR" ]; then
        echo "Error: Plugin directory '$PLUGIN_DIR' not found"
        exit 1
    fi

    if [ ! -f "$PLUGIN_DIR/Cargo.toml" ]; then
        echo "Error: No Cargo.toml found in '$PLUGIN_DIR'"
        exit 1
    fi

    if [ ! -f "$PLUGIN_DIR/plugin.toml" ]; then
        echo "Error: No plugin.toml found in '$PLUGIN_DIR'"
        echo "Create one with: name, version, description, min_interface_version"
        exit 1
    fi

    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo " Building $PLUGIN..."
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    cd "$PLUGIN_DIR" && cargo build --release
    cd ..

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo " Deploying to $PLUGINS_DEST"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # Create destination directory
    mkdir -p "$PLUGINS_DEST"

    # Determine library extension based on OS
    if [[ "$OSTYPE" == "darwin"* ]]; then
        LIB_EXT="dylib"
    elif [[ "$OSTYPE" == "linux"* ]]; then
        LIB_EXT="so"
    elif [[ "$OSTYPE" == "msys"* ]] || [[ "$OSTYPE" == "win32"* ]]; then
        LIB_EXT="dll"
    else
        echo "Warning: Unknown OS type '$OSTYPE', assuming .so extension"
        LIB_EXT="so"
    fi

    # Convert plugin name to library name (hyphens to underscores)
    LIB_NAME="lib${PLUGIN//-/_}.$LIB_EXT"
    LIB_PATH="$PLUGIN_DIR/target/release/$LIB_NAME"

    if [ ! -f "$LIB_PATH" ]; then
        echo "Error: Built library not found at '$LIB_PATH'"
        exit 1
    fi

    # Copy files
    cp "$PLUGIN_DIR/plugin.toml" "$PLUGINS_DEST/"
    cp "$LIB_PATH" "$PLUGINS_DEST/"

    echo "✓ Copied plugin.toml"
    echo "✓ Copied $LIB_NAME"

    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo " Deployed successfully!"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "Contents of $PLUGINS_DEST:"
    ls -la "$PLUGINS_DEST/"
    echo ""
    echo "To test:"
    echo "  1. Start totui (or restart if already running)"
    echo "  2. Press 'P' to open plugins menu"
    echo "  3. Enable '$PLUGIN' if not already enabled"
    echo ""
    echo "To iterate:"
    echo "  just test-deploy $PLUGIN"
