# pupi

Blazingly fast meta-build tool for open-source libraries.

## Overview

`pupi` is a meta-build tool designed to manage and coordinate builds across multiple packages in a monorepo setup. It supports both Rust (Cargo) and JavaScript/TypeScript (npm) projects, handling dependency management, version synchronization, and build orchestration.

## Installation

```bash
cargo install portal-solutions-pupi
```

Or build from source:

```bash
cargo build --release
```

## Configuration

`pupi` uses a configuration file named `pupi.json` or `pupi.yaml`/`pupi.yml` in the root of your project. YAML is supported as an alternative to JSON for easier readability and editing.

### Configuration File Locations

- `pupi.json` - JSON format (checked first)
- `pupi.yaml` - YAML format
- `pupi.yml` - YAML format (alternative extension)

**Note:** `package.json` files are always in JSON format and are not affected by the YAML configuration option.

### JSON Schema

A JSON schema for the configuration file can be generated using:

```bash
pupi schema > pupi-schema.json
```

You can also find the schema in [`pupi-schema.json`](./pupi-schema.json).

### Configuration Structure

The configuration file defines workspace members and their properties:

```yaml
# Example pupi.yaml configuration
my-package:
  version: "1.0.0"
  description: "My package description"
  deps:
    dependency-name: {}
  cargo: {}      # Enable Cargo/Rust support
  npm: {}        # Enable npm/JavaScript support
  private: false # Whether to publish this package
```

#### Member Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `version` | string | Yes | Package version |
| `description` | string | Yes | Package description |
| `deps` | object | Yes | Dependencies on other workspace members |
| `cargo` | object | No | Cargo/Rust configuration (presence enables Rust support) |
| `npm` | object | No | npm configuration (presence enables npm support) |
| `private` | boolean | No | Whether to skip publishing (default: false) |
| `parent` | string | No | Parent package path |
| `subtree` | object | No | Git subtree configuration |
| `updater` | array | No | Custom update script |

## Usage

### Setup

Initialize a new project workspace:

```bash
pupi setup <root_path>
```

This will:
- Initialize a git repository if not present
- Create `pupi.json` if not present
- Create `package.json` with workspace configuration
- Create `Cargo.toml` with workspace configuration
- Install required npm dev dependencies (parcel, zshy, typescript, etc.)

### Generate Schema

Generate the JSON schema for configuration files:

```bash
pupi schema
```

### Build

Build all packages in the workspace:

```bash
pupi build <root_path>
```

### Publish

Publish all non-private packages:

```bash
pupi publish <root_path>
```

### Autogen

Run automatic generation/update for packages:

```bash
pupi autogen <root_path>
```

### Update

Update package configurations and dependencies:

```bash
pupi update <root_path>
```

## Example Configuration

### JSON Format (pupi.json)

```json
{
  "my-library": {
    "version": "1.0.0",
    "description": "A sample library",
    "deps": {},
    "cargo": {},
    "npm": {}
  },
  "my-app": {
    "version": "1.0.0", 
    "description": "An application using my-library",
    "deps": {
      "my-library": {}
    },
    "npm": {}
  }
}
```

### YAML Format (pupi.yaml)

```yaml
my-library:
  version: "1.0.0"
  description: "A sample library"
  deps: {}
  cargo: {}
  npm: {}

my-app:
  version: "1.0.0"
  description: "An application using my-library"
  deps:
    my-library: {}
  npm: {}
```

## License

MPL-2.0
