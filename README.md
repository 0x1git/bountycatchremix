# BountyCatch Remix 🎯

A high-performance bug bounty domain management tool for security researchers and penetration testers. Written in **Rust** for maximum speed and efficiency.

This repository contains *my remix* of Jason Haddix's [`bountycatch.py`](https://gist.github.com/jhaddix/91035a01168902e8130a8e1bb383ae1e) script. The original script was simple and easier to manage—I rewrote it in Rust with PostgreSQL backend for handling millions of domains at blazing speed 🚀.

*(Note: courtesy of this script goes to Jason Haddix. I just added some features that I wanted there and maintaining the core simplicity ❤️)*

## Overview

**BountyCatch** is a CLI application for managing domain lists in bug bounties. It provides domain validation, duplicate detection, multiple export formats, and **PostgreSQL-backed storage**. All domains are stored in a single collection (no per-project flag needed).

## Performance

All operations use PostgreSQL COPY protocol by default for maximum speed:

| Operation | Domains | Time | Throughput |
|-----------|---------|------|------------|
| Add | 1,000,000 | ~3.4s | ~294K/sec |
| Add | 21,637,355 | ~122s | ~177K/sec |
| Count | 21,000,000 | ~0.1s | instant |
| Print | 1,000,000 | ~0.4s | ~2.5M/sec |
| Export | 1,000,000 | ~1.2s | ~833K/sec |

## Features

### ✨ **Domain Management**
- **Domain validation** with a comprehensive regex that supports:
  - Leading wildcards: `*.example.com`
  - Internal wildcards: `svc-*.domain.com`, `rac-*.net.dell.com`
  - Service records (underscore labels): `_service.domain.com`
  - Standard domains/subdomains: `example.com`, `sub.domain.com`
- **Automatic duplicate detection** via PostgreSQL PRIMARY KEY constraint
- **Bulk import** from text files or stdin with validation feedback
- **Multiple export formats** (text and JSON with metadata)
- **Filtering** with `--match` (substring) or `--regex` patterns
- **Domain removal** with filters for cleaning up domains

### 🔧 **Technical Features**
- **Written in Rust** - compiled native binary, no runtime dependencies
- **PostgreSQL storage** - reliable, persistent, handles 10M+ domains
- **Connection pooling** - efficient database connections with deadpool
- **Async I/O** - tokio-based async runtime for high throughput
- **COPY protocol** - PostgreSQL COPY for bulk operations (~175K domains/sec)
- **Stdin support** - pipe domains directly: `echo "domain.com" | bountycatch add`
- **Silent mode** - `-s` flag suppresses logs for clean piped output
- **Auto-config detection** - finds config.json from standard locations
- **Environment variable overrides** for containerized deployments

### 📊 **Export & Filtering**
- **JSON export** with metadata and timestamps
- **Text export** for integration with other tools
- **Substring filtering**: `--match .dell.com`
- **Regex filtering**: `--regex '.*\.dell\.com$'`
- **Sorted output**: `--sort` flag

## Installation

### Prerequisites

- PostgreSQL 12+

### Quick Setup

```bash
# Clone the repository
git clone https://github.com/0x1git/bountycatchremix.git
cd bountycatchremix

# Build the Rust binary
cd rust
cargo build --release

# Set up PostgreSQL (run with sudo)
cd ..
sudo ./setup_postgres.sh

# Install system-wide 
sudo cp rust/target/release/bountycatch /usr/local/bin/bountycatch

# Copy config to user directory
mkdir -p ~/.config/bountycatch
cp config.json ~/.config/bountycatch/
```

### Building from Source

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build release binary
cd rust
cargo build --release

# Binary will be at: rust/target/release/bountycatch
```

### Installing PostgreSQL

#### **Linux (Ubuntu/Debian/Kali)**
```bash
sudo apt update
sudo apt install postgresql postgresql-contrib -y
sudo systemctl start postgresql
sudo systemctl enable postgresql
```

#### **Linux (RHEL/CentOS/Fedora)**
```bash
sudo dnf install postgresql-server postgresql-contrib
sudo postgresql-setup --initdb
sudo systemctl start postgresql
sudo systemctl enable postgresql
```

#### **macOS**
```bash
brew install postgresql@15
brew services start postgresql@15
```

#### **Windows**
Download from: https://www.postgresql.org/download/windows/

## Configuration

### Config File Locations
The tool auto-detects `config.json` from these locations (in order):
1. `~/.config/bountycatch/config.json` (XDG standard - recommended)
2. `~/.bountycatch/config.json`
3. `/etc/bountycatch/config.json` (system-wide)
4. Current directory (for development)

### Default Configuration
```json
{
  "postgresql": {
    "host": "localhost",
    "port": 5432,
    "database": "bountycatch",
    "user": "postgres",
    "password": "your_password",
    "min_connections": 1,
    "max_connections": 10
  }
}
```

### Environment Variables
Override settings with environment variables:
```bash
export PGHOST=my-postgres-server
export PGPORT=5432
export PGDATABASE=bountycatch
export PGUSER=postgres
export PGPASSWORD=mypassword
```

## Usage

### Command Structure
```bash
bountycatch [global-options] <command> [command-options]
```

### Global Options
| Option | Description |
|--------|-------------|
| `-c, --config` | Specify configuration file path |
| `-s, --silent` | Suppress console logs; only emit command output |
| `-h, --help` | Show help message |
| `-V, --version` | Show version |

### Commands

#### **Adding Domains**

```bash
# From file
bountycatch add -f domains.txt

# From stdin (pipe from other tools)
echo "example.com" | bountycatch add
subfinder -d example.com -silent | bountycatch add
cat domains.txt | bountycatch -s add

# Skip validation for raw input
bountycatch add -f raw.txt --no-validate
```

> **Performance**: Uses PostgreSQL COPY protocol with index rebuilding for 
> maximum throughput (~175K domains/sec at scale, faster for smaller batches).

#### **Printing Domains**

```bash
# Print all domains
bountycatch print

# Silent mode (no logs, just domains)
bountycatch -s print

# With substring filter
bountycatch -s print --match .dell.com

# With regex filter
bountycatch -s print --regex '.*\.dell\.com$'

# Sorted output
bountycatch -s print --match .dell.com --sort

# Pipe to other tools
bountycatch -s print | nuclei -t takeovers/
bountycatch -s print --match .example.com | httpx -silent
```

#### **Counting Domains**

```bash
# Count all domains
bountycatch count

# Count with filter
bountycatch -s count --match .dell.com
bountycatch -s count --regex '\.gov$'
```

#### **Exporting Domains**

```bash
# Export to text file
bountycatch export -f domains.txt

# Export to JSON with metadata
bountycatch export -f domains.json --format json

# Export with filter
bountycatch export -f dell-domains.txt --match .dell.com
bountycatch export -f gov-domains.json --format json --regex '\.gov$'

# Sorted export
bountycatch export -f sorted.txt --sort
```

#### **Removing Domains**

```bash
# Remove from stdin (pipe from other tools)
echo "unwanted.example.com" | bountycatch remove
cat domains_to_remove.txt | bountycatch remove

# Remove a single domain
bountycatch remove -d unwanted-domain.com

# Remove from file
bountycatch remove -f domains_to_remove.txt

# Remove by substring filter
bountycatch remove --match .old-domain.com

# Remove by regex
bountycatch remove --regex '.*\.test\.com$'
```

#### **Deleting All Domains**

```bash
# With confirmation prompt
bountycatch delete-all

# Skip confirmation (use in scripts)
bountycatch delete-all --confirm
```

### Pipeline Examples

```bash
# Subdomain enumeration → storage
subfinder -d example.com -silent | bountycatch -s add
amass enum -d example.com | bountycatch -s add

# Storage → vulnerability scanning
bountycatch -s print --match .example.com | httpx -silent 

# Filter and process specific targets
bountycatch -s print --match api. | httpx -silent -mc 200

# Export for external tools
bountycatch export -f targets.txt --match .prod
```

## Input File Format

### Domain List (domains.txt)
```
example.com
api.example.com
*.wildcard.example.com
_service.example.com
subdomain.example.org
```

### Validation Rules
**Valid inputs:**
- Leading wildcard: `*.example.com`
- Internal wildcard: `svc-*.domain.com`, `rac-*.net.dell.com`
- Service record (underscore): `_service.domain.com`
- Standard domain/subdomain: `example.com`, `sub.domain.com`

**Invalid (will be skipped):**
- `*abc.com` (invalid wildcard without dot)
- `svc-*` (no TLD)
- `-.example.com` (invalid label)
- `http://example.com` (protocols not supported)

## Export Formats

### Text Format
```
api.example.com
example.com
subdomain.example.org
```

### JSON Format
```json
{
  "domain_count": 3,
  "exported_at": "2026-01-31T22:59:04.762184",
  "domains": [
    "api.example.com",
    "example.com",
    "subdomain.example.org"
  ]
}
```

## Troubleshooting

### Common Errors

**Connection refused:**
```bash
# Check if PostgreSQL is running
sudo systemctl status postgresql

# Start if needed
sudo systemctl start postgresql
```

**Authentication failed:**
```bash
# Verify password in config
cat ~/.config/bountycatch/config.json

# Or use environment variable
export PGPASSWORD=yourpassword
```

**Permission denied:**
```bash
# Run setup script with sudo
sudo ./setup_postgres.sh
```

### Tips
1. Use `-s` for clean output when piping to other tools
2. All operations use fast COPY protocol by default
3. Check PostgreSQL logs for detailed error messages

---
Happy hunting folks! 🕵️‍♂️

## License

MIT
