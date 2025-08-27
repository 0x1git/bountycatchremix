# BountyCatch Remix 🎯

A bug bounty domain management tool for security researchers and penetration testers. This repository contains *my remix* of Jason Haddix's  [`bountycatch.py`](https://gist.github.com/jhaddix/91035a01168902e8130a8e1bb383ae1e) script. The original script was simple and easier to manage, and I just added my own twist so it could do other commands I needed 🧸.


*(Note: courtesy of this script goes to Jason Haddix. I just added some features that I wanted there and maintaining the core simplicity ❤️)*


## Overview

**BountyCatch** is a simple Python application for managing domain lists in bug bounties. It provides domain validation, duplicate detection, multiple export formats, and Redis-backed storage with connection pooling. All domains are stored in a single collection (no per-project flag needed).

## Features

### ✨ **Domain Management**
- **Domain validation** with a comprehensive regex that supports:
  - Leading wildcards: `*.example.com`
  - Internal wildcards: `svc-*.domain.com`, `rac-*.net.dell.com`, `test.*.invalid.com`
  - Service records (underscore labels): `_service.domain.com`, `_collab-edge.5g.dell.com`
  - Standard domains/subdomains: `example.com`, `sub.domain.com`
- **Automatic duplicate detection** and statistics reporting
- **Bulk import** from text files with validation feedback
- **Multiple export formats** (text and JSON with metadata)
- **Domain removal** for cleaning up incorrectly added domains
- **List/print** and **count** domains

### 🔧 **New Features**
- **Configuration file support** with environment variable overrides
- **Better logging** to both console and file
- **Redis connection pooling** for optimal performance
- **Better error handling** with graceful failure recovery

### 📊 **Export & Analytics**
- **JSON export** with metadata and timestamps
- **Text export** for integration with other tools
- **Domain statistics** and duplicate reporting
- **Database counting** capability

## Installation

### Prerequisites

You'll need **Redis** installed and running on your system.

### Installing Redis

#### **Linux (Ubuntu/Debian)**
```bash
sudo apt update
sudo apt install redis-server redis-tools

# Start Redis service
sudo systemctl start redis
sudo systemctl enable redis
```

#### **Linux (RHEL/CentOS/Fedora)**
```bash
sudo dnf install redis
# or for older systems: sudo yum install redis

# Start Redis service
sudo systemctl start redis
sudo systemctl enable redis
```

#### **Windows**
```powershell
# Using Chocolatey
choco install redis-64

# Or download from: https://github.com/microsoftarchive/redis/releases
# Then run: redis-server.exe
```

#### **macOS**
```bash
brew install redis
brew services start redis
```

### Python Dependencies
```bash
pip install redis
# or
pip install -r requirements.txt
```

## Configuration

### Default Configuration
Create `config.json` for custom Redis settings:
```json
{
  "redis": {
    "host": "localhost",
    "port": 6379,
    "db": 0,
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
export REDIS_HOST=my-redis-server
export REDIS_PORT=6380
```

## Usage

### Command Structure
```bash
python3 bountycatch.py [global-options] <command> [command-options]
```

### Global Options
- `-c, --config CONFIG` - Specify configuration file path
- `-v, --verbose` - Enable verbose (DEBUG) logging
- `-h, --help` - Show help message

### Commands

#### **Adding Domains**
Import domains from a text file with automatic validation:
```bash
python3 bountycatch.py add -f domains.txt

# Skip domain validation (not recommended)
python3 bountycatch.py add -f domains.txt --no-validate
```

#### **Counting Domains**
Get the total number of domains in the database:
```bash
python3 bountycatch.py count
```

#### **Listing Domains**
Print all domains in alphabetical order:
```bash
python3 bountycatch.py print
```

#### **Removing Domains**
Remove domains that were added by mistake:
```bash
# Remove a single domain
python3 bountycatch.py remove -d unwanted-domain.com

# Remove multiple domains from a file
python3 bountycatch.py remove -f domains_to_remove.txt
```

#### **Exporting Domains**
Export domains to various formats:
```bash
# Export to text file (default)
python3 bountycatch.py export -f domains.txt

# Export to JSON with metadata
python3 bountycatch.py export -f domains.json --format json
```

#### **Deleting All Domains**
Remove all domains from the database:
```bash
# With confirmation prompt
python3 bountycatch.py delete-all

# Skip confirmation (use with caution)
python3 bountycatch.py delete-all --confirm
```

### Other Usage

#### **Custom Configuration**
```bash
python3 bountycatch.py -c my-config.json -v add -f domains.txt
```

#### **Batch Operations**
```bash
# Process multiple files
for file in *.txt; do
  python3 bountycatch.py add -f "$file"
done
```

## Input File Format

### Domain List (domains.txt)
```
example.com
api.example.com
subdomain.example.org
test.co.uk
```

### Domain Removal List (domains_to_remove.txt)
```
unwanted-domain.com
old-subdomain.example.com
mistake.org
```

### New: Validation Rules
- Supported valid inputs:
  - Leading wildcard: `*.example.com`
  - Internal wildcard: `svc-*.domain.com`, `rac-*.net.dell.com`, `test.*.invalid.com`
  - Service record (underscore): `_service.domain.com`, `_collab-edge.5g.dell.com`
  - Standard domain/subdomain: `example.com`, `sub.domain.com`
- Explicitly invalid and will be skipped:
  - `*abc.com` (invalid leading wildcard without dot)
  - `svc-*` (no TLD)
  - `-.example.com` (invalid empty label)
- Empty lines are ignored
- Protocols (http://, https://) and paths are not supported and will be rejected

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
  "exported_at": "2025-06-05T20:29:54.867184",
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
- **Console** - Real-time feedback
- **File** - `bountycatch.log` for persistent logging

## Error Handling

### Common Errors

- **Connection errors**: Check if Redis is running and accessible.
- **File not found**: Ensure the file paths are correct.
- **Permission denied**: Check file and directory permissions.

### Domain Removal
- **Domain not found**: Warns when trying to remove domains that don't exist in the project
- **File not found**: Graceful handling when removal file doesn't exist
- **Statistics reporting**: Shows count of successfully removed vs. not found domains

### Database Management
- **Redis connection issues**: Graceful failure with helpful error messages
- **Invalid domain validation**: Automatic skipping with detailed logging

### Troubleshooting Tips

1. **Verbose logging**: Use the `-v` option for detailed logs.
2. **Check Redis logs**: For connection-related issues.
3. **Validate input files**: Ensure they meet the required format and permissions.

---
Happy hunting folks! 🕵️‍♂️

## Licence
