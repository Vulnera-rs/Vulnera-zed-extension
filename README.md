# Vulnera Zed Extension

Production-ready Zed wrapper for the Vulnera Language Server (LSP), delivering vulnerability diagnostics and quick fixes for dependency manifests.

- Auto-installs and caches the Node-based language server on first run.
- Configurable via Zed settings and environment variables.
- Integrates with Zed features: diagnostics, code actions, hover, etc.
- Tunable performance with sensible defaults.

---

## Contents

- [What It Does](#what-it-does)
- [Supported Manifests](#supported-manifests)
- [Requirements](#requirements)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
  - [Language Associations](#language-associations)
  - [LSP Initialization Options vs Runtime Settings](#lsp-initialization-options-vs-runtime-settings)
  - [Example Zed settings.json](#example-zed-settingsjson)
  - [Environment Variables](#environment-variables)
- [Performance Tuning](#performance-tuning)
- [First-Run Auto-Install](#first-run-auto-install)
  - [Install Location](#install-location)
  - [Version Pinning](#version-pinning)
  - [Offline/Enterprise Environments](#offlineenterprise-environments)
  - [Behind a Proxy](#behind-a-proxy)
- [Advanced: Binary Overrides](#advanced-binary-overrides)
- [Troubleshooting](#troubleshooting)
- [Uninstall / Reset](#uninstall--reset)
- [Security Notes](#security-notes)
- [Releases and Publishing](#releases-and-publishing)
- [License](#license)

---

## What It Does

This extension launches and manages the Vulnera Language Server (a Node.js process speaking LSP over stdio). It attaches to manifest-like files and surfaces:

- Inline diagnostics with severity mapping
- Quick fixes (code actions) to upgrade to fixed versions
- Hover info and other LSP features supported by your Zed setup

The wrapper takes care of installing and updating the LSP package, version pinning, and consistent dependency resolution.

---

## Supported Manifests

- Python: requirements.txt, Pipfile, pyproject.toml
- Node.js: package.json
- PHP: composer.json
- Rust: Cargo.toml
- Go: go.mod
- Java: pom.xml, build.gradle
- Ruby: Gemfile
- .NET: *.csproj, *.sln

Lockfiles (e.g. package-lock.json, yarn.lock, composer.lock, Cargo.lock, go.sum) can be analyzed but are never edited by quick fixes.

Note: The LSP filters by filename at runtime. You can safely attach it to broader language groups (JSON/TOML/XML/Groovy/etc.) without affecting non-manifest files.

---

## Requirements

- Zed editor (latest recommended)
- Node.js 18+ and npm available on PATH for first-run auto-install
- Network access to your Vulnera API endpoint 

After the first run, the server is cached locally and does not require network to start.

---

## Quick Start

1. Install as a Dev Extension in Zed:

- Open Zed → Extensions → Install Dev Extension → select the `vulnera_zed_extension/` folder.
- Zed will build and register the extension.

2. Configure Zed:

- Add the Vulnera LSP to the languages you care about (see [Language Associations](#language-associations)) and set API settings (see [Configuration](#configuration)).

3. Open a supported manifest:

- Diagnostics will appear. Use code actions to apply available fixes.

Optional: To auto-install this extension when opening relevant files, add it to your `auto_install_extensions` in Zed settings.

---

## Configuration

### Language Associations

Attach the “vulnera” language server to languages commonly used for dependency manifests. The LSP filters by filename, so attaching to JSON/TOML/XML/Groovy/etc. is safe.

Recommended languages:

- JSON, TOML, XML, Groovy, Go, Python
- Add others as you see fit (e.g., Java)

### LSP Initialization Options vs Runtime Settings

- initialization_options: Applied only at startup. Changing these requires restarting the language server.
- settings: Applied at runtime. Changing these does not require restart.

For options that a server must read at startup (like certain feature flags), use initialization_options. For normal behavior toggles and thresholds, use settings.

### Example Zed settings.json

Below is a sensible configuration that attaches Vulnera broadly and configures both startup and runtime options. Adjust as needed.

```json
{
  "auto_install_extensions": ["vulnera"],
  "lsp_fetch_timeout_ms": 40000,

  "languages": {
    "JSON": { "language_servers": ["vulnera"] },
    "TOML": { "language_servers": ["vulnera"] },
    "XML": { "language_servers": ["vulnera"] },
    "Groovy": { "language_servers": ["vulnera"] },
    "Go": { "language_servers": ["vulnera"] },
    "Python": { "language_servers": ["vulnera"] }
  },

  "lsp": {
    "vulnera": {
      "initialization_options": {
        "vulnera": {
          "apiBaseUrl": "http://localhost:3000"
        }
      },
      "settings": {
        "vulnera": {
          "apiBaseUrl": "http://localhost:3000",
          "analyzeOnOpen": true,
          "analyzeOnSave": false,
          "severityMin": "Low",
          "includeLockfiles": true,
          "requestTimeoutMs": 10000
        }
      }

      /*
      Advanced: You can explicitly point to a local server build and bypass auto-install.
      See “Advanced: Binary Overrides” below.
      */
    }
  }
}
```

### Environment Variables

- VULNERA_API_BASE_URL: Overrides the API base URL for the LSP (highest precedence for local dev).
- VULNERA_LSP_VERSION: Pin the @vulnera/language-server version for this machine (e.g., 1.2.3). If a different version is installed, the wrapper triggers a reinstall.
- VULNERA_LSP_VERSION_DEFAULT: Set at extension build time to bake a default LSP version into the wrapper; used unless overridden by VULNERA_LSP_VERSION.

### Using Your Language Server Binary

The extension looks for `vulnera-language-server` in your system PATH. If you have it installed, it should work automatically.

If you need to specify a custom path or the binary has a different name, configure it in your Zed settings.json:

```json
{
  "lsp": {
    "vulnera": {
      "binary": {
        "path": "/usr/bin/vulnera-language-server",
        "arguments": ["--stdio"]
      }
    }
  }
}
```

---

## Performance Tuning

- Completion/Request timeouts: Use Zed’s `lsp_fetch_timeout_ms` (recommended 40000 ms) to cap how long the editor waits for LSP responses.
- Highlight debounce: Tune Zed’s highlight debounce setting (the delay before requesting highlights based on the cursor). A small increase can reduce chatter on very large files.
- The Vulnera LSP’s own requestTimeoutMs setting (in LSP “settings”) controls server-side timeouts for backend requests.

These values depend on project size and network latency. Start with defaults; raise or lower based on responsiveness.

---
### Version Pinning

- Runtime pinning: Set `VULNERA_LSP_VERSION=1.2.3` to install/use that exact version. If a different version is detected, the wrapper triggers a reinstall.
- Compile-time default: Build the extension with `VULNERA_LSP_VERSION_DEFAULT=1.2.3` to bake a default into the wrapper binary. Users can still override with `VULNERA_LSP_VERSION`.

### Offline/Enterprise Environments

- Pre-populate: You can stage the install directory with a pre-installed `node_modules/@vulnera/language-server` matching your desired version. The wrapper will use it without network.
- Airgapped upgrades: Update by replacing the install dir contents with the new version.

### Behind a Proxy

- The installer uses `npm install --prefix <dir>`. Ensure your environment/npm config has proxy settings configured (`HTTP(S)_PROXY`, `.npmrc`, or enterprise npm registry).

---

## Advanced: Binary Overrides

You can explicitly point Zed to your language server binary (bypassing auto-install). This is useful for local LSP development or enterprise-locked environments:

**For a native binary:**
```json
{
  "lsp": {
    "vulnera": {
      "binary": {
        "path": "/usr/bin/vulnera-language-server",
        "arguments": ["--stdio"],
        "env": {
          "VULNERA_API_BASE_URL": "http://localhost:3000"
        }
      }
    }
  }
}
```

**For a Node.js server:**
```json
{
  "lsp": {
    "vulnera": {
      "binary": {
        "path": "node",
        "arguments": [
          "/absolute/path/to/packages/vulnera-language-server/dist/server.js",
          "--stdio"
        ],
        "env": {
          "VULNERA_API_BASE_URL": "http://localhost:3000"
        }
      }
    }
  }
}
```

---

## Troubleshooting

- Server never starts:
  - Ensure npm is available on PATH for the first run.
  - Check the Zed log (“zed: open log” or run Zed with `--foreground`).
  - Verify Node 18+ is available. The wrapper prefers Zed’s bundled Node for running the server.

- No diagnostics:
  - Confirm the file is a supported manifest.
  - Ensure the Vulnera LSP is attached to the file’s language in Zed settings.
  - Verify `apiBaseUrl` points to a reachable endpoint.

- Install errors:
  - Check proxy settings (npm uses your environment/.npmrc).
  - Clear the install directory (see Uninstall/Reset) and restart Zed.
  - Pin a specific version via `VULNERA_LSP_VERSION` if `latest` causes resolution issues.

- Version mismatch loop:
  - If you set `VULNERA_LSP_VERSION`, ensure the installed package matches. Delete the install dir to force a clean reinstall.

- Logs and stdio:
  - The installer discards npm stdout and passes only stderr to avoid corrupting the LSP stdio stream. Server logs are visible in Zed’s log.

---


## Security Notes

- Installation is scoped to a per-user directory. No global writes.
- The wrapper avoids shell “eval” and uses `npm install --prefix` directly.
- Environment variables can configure endpoints; do not embed secrets in logs or settings where not appropriate for your environment.

---

## License

MIT
