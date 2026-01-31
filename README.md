# BountyCatch Remix 🎯

A bug bounty domain management tool for security researchers and penetration testers. This repository contains *my remix* of Jason Haddix's [`bountycatch.py`](https://gist.github.com/jhaddix/91035a01168902e8130a8e1bb383ae1e) script. The original script was simple and easier to manage, and I just added my own twist so it could do other commands I needed 🧸.

*(Note: courtesy of this script goes to Jason Haddix. I just added some features that I wanted there and maintaining the core simplicity ❤️)*

## Overview

**BountyCatch** is a simple Python application for managing domain lists in bug bounties. It provides domain validation, duplicate detection, multiple export formats, and **PostgreSQL-backed storage** with connection pooling. All domains are stored in a single collection (no per-project flag needed).

## Features

### ✨ **Domain Management**
- **Domain validation** with a comprehensive regex that supports:
  - Leading wildcards: `*.example.com`
  - Internal wildcards: `svc-*.domain.com`, `rac-*.net.dell.com`, `test.*.invalid.com`
  - Service records (underscore labels): `_service.domain.com`, `_collab-edge.5g.dell.com`
  - Standard domains/subdomains: `example.com`, `sub.domain.com`
- **Automatic duplicate detection** via PostgreSQL PRIMARY KEY constraint
- **Bulk import** from text files or stdin with validation feedback
- **Multiple export formats** (text and JSON with metadata)
- **Filtering** with `--match` (substring) or `--regex` patterns
- **Domain removal** with filters for cleaning up domains

### 🔧 **Features**
- **PostgreSQL storage** - reliable, persistent, handles 10M+ domains
- **Stdin support** - pipe domains directly: `echo "domain.com" | bountycatch add`
- **Streaming output** - server-side cursors for memory-efficient iteration
- **Silent mode** - `-s` flag suppresses logs for clean piped output
- **Auto-config detection** - finds config.json from standard locations
- **Environment variable overrides** for containerized deployments

### 📊 **Export & Filtering**
- **JSON export** with metadata and timestamps
- **Text export** for integration with other tools
- **Substring filtering**: `--match .dell.com`
- **Regex filtering**: `--regex '.*\.dell\.com$'`
- **Sorted output**: `--sort` flag (slower for large datasets)

## Installation

### Prerequisites

- Python 3.8+
- PostgreSQL 12+

### Quick Setup

```bash
# Clone the repository
git clone https://github.com/0x1git/bountycatchremix.git
cd bountycatchremix

# Install Python dependencies
pip install -r requirements.txt

# Set up PostgreSQL (run with sudo)
sudo ./setup_postgres.sh

# Install system-wide 
sudo cp bountycatch.py /usr/local/bin/bountycatch
sudo chmod +x /usr/local/bin/bountycatch

# Copy config to user directory
mkdir -p ~/.config/bountycatch
cp config.json ~/.config/bountycatch/
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

### Python Dependencies
```bash
pip install psycopg2-binary
# or
pip install -r requirements.txt
```

## Configuration

### Config File Locations
The tool auto-detects `config.json` from these locations (in order):
1. `~/.config/bountycatch/config.json` (XDG standard - recommended)
2. `~/.bountycatch/config.json`
3. `/etc/bountycatch/config.json` (system-wide)
4. Script directory (for development)

### Default Configuration
```json
{
  "postgresql": {
    "host": "localhost",
    "port": 5432,
    "database": "bountycatch",
    "user": "postgres",
    "password": "228899",
    "min_connections": 1,
    "max_connections": 10
  },
  "logging": {
    "level": "INFO",
    "format": "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
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
| `-v, --verbose` | Enable verbose (DEBUG) logging |
| `-s, --silent` | Suppress console logs; only emit command output |
| `-h, --help` | Show help message |

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

# Sorted output (slower for large datasets)
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

## Logging

### Log Levels
- `DEBUG` - Verbose debugging information
- `INFO` - General operational messages
- `WARNING` - Important notices (invalid domains, etc.)
- `ERROR` - Error conditions

### Log Destinations
- **Console** - Real-time feedback (suppressed with `-s`)
- **File** - `bountycatch.log` for persistent logging

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
export PGPASSWORD=pass
```

**Permission denied:**
```bash
# Run setup script with sudo
sudo ./setup_postgres.sh
```

### Tips
1. Use `-v` for verbose logging to debug issues
2. Use `-s` for clean output when piping to other tools
3. Check `bountycatch.log` for detailed error messages

---
Happy hunting folks! 🕵️‍♂️

## License

MIT
