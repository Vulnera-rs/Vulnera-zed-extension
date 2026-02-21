# Vulnera for Zed

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)

**Vulnera** is a high-performance, multi-ecosystem vulnerability analysis extension for the [Zed editor](https://zed.dev). It delivers real-time security diagnostics and automated remediation for dependency manifests directly in your editor.

This extension manages the **Vulnera Language Server**, providing seamless integration with Zed's diagnostics, code actions, and hover features.

## Features

- **Seamless Setup**: Auto-installs and caches the Node-based language server on the first run.
- **Inline Diagnostics**: Instant feedback on vulnerable dependencies with severity mapping.
- **Automated Fixes**: Integrated Code Actions to upgrade to fixed versions with a single click.
- **Broad Ecosystem Support**:
  - **Rust**: `Cargo.toml`, `Cargo.lock`
  - **Python**: `requirements.txt`, `Pipfile`, `pyproject.toml`
  - **Node.js**: `package.json`, `package-lock.json`, `yarn.lock`
  - **Go**: `go.mod`, `go.sum`
  - **Java**: `pom.xml`, `build.gradle`
  - **PHP**: `composer.json`, `composer.lock`
  - **Ruby**: `Gemfile`, `Gemfile.lock`
  - **.NET**: `*.csproj`, `*.sln`, `packages.config`

## Quick Start

1.  **Install the Extension**:
    - Open Zed.
    - Go to the Extensions view and search for **Vulnera**.
    - Click **Install**.
2.  **Configuration**: Add the server to your `settings.json` (see [Configuration](#configuration) below).
3.  **Analyze**: Open any supported manifest file. Vulnera will automatically begin scanning and report findings in the diagnostics tray.

## Configuration

Configure the Vulnera LSP in your Zed `settings.json`. The extension attaches to files based on their language type, but the underlying LSP intelligently filters by filename.

### Example `settings.json`

```json
{
  "lsp": {
    "vulnera": {
      "initialization_options": {
        "vulnera": {
          "apiBaseUrl": "https://api.vulnera.studio"
        }
      },
      "settings": {
        "vulnera": {
          "apiBaseUrl": "https://api.vulnera.studio",
          "analyzeOnOpen": true,
          "analyzeOnSave": false,
          "severityMin": "High",
          "includeLockfiles": true
        }
      }
    }
  },
  "languages": {
    "JSON": { "language_servers": ["vulnera"] },
    "TOML": { "language_servers": ["vulnera"] },
    "Go": { "language_servers": ["vulnera"] },
    "Python": { "language_servers": ["vulnera"] }
  }
}
```

### Settings Reference

| Key                | Default                      | Description                                          |
| :----------------- | :--------------------------- | :--------------------------------------------------- |
| `apiBaseUrl`       | `https://api.vulnera.studio` | The Vulnera API endpoint.                            |
| `analyzeOnOpen`    | `true`                       | Trigger scan when a manifest is opened.              |
| `analyzeOnSave`    | `false`                      | Trigger scan when a manifest is saved.               |
| `severityMin`      | `High`                       | Filter results: `Low`, `Medium`, `High`, `Critical`. |
| `includeLockfiles` | `true`                       | Include lockfiles in workspace-wide analysis.        |

## Requirements

- **Zed Editor**: Latest version recommended.
- **Node.js**: `v18.0.0` or higher (available on `PATH` for the initial installation).
- **Network**: Access to the Vulnera API (default: `api.vulnera.studio`).

## Advanced: Custom Binary

If you wish to use a locally compiled version of the `vulnera-language-server`, you can override the binary path:

```json
{
  "lsp": {
    "vulnera": {
      "binary": {
        "path": "/usr/local/bin/vulnera-language-server",
        "arguments": ["--stdio"]
      }
    }
  }
}
```

## Troubleshooting

- **No Diagnostics**: Ensure the file extension is associated with a language that has `vulnera` enabled in `settings.json`.
- **Installation Failed**: Check that `npm` is available in your terminal. The extension uses `npm` to download the language server on first run.
- **Logs**: You can view the LSP logs by running the `zed: open log` command and looking for the Vulnera process output.

## License

MIT

---

**Developed with ❤️ by the Vulnera Team.**
