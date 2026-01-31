#!/bin/bash
# PostgreSQL setup script for bountycatch
# Run with sudo: sudo ./setup_postgres.sh

set -e

DB_NAME="bountycatch"
DB_USER="postgres"
DB_PASS="228899"

echo "[*] Setting up PostgreSQL for bountycatch..."

# Check if PostgreSQL is installed
if ! command -v psql &> /dev/null; then
    echo "[*] Installing PostgreSQL..."
    apt-get update -qq
    apt-get install -y postgresql postgresql-contrib
fi

# Start PostgreSQL if not running
if ! systemctl is-active --quiet postgresql; then
    echo "[*] Starting PostgreSQL service..."
    systemctl start postgresql
    systemctl enable postgresql
fi

# Set password for postgres user and create database
echo "[*] Configuring PostgreSQL..."
sudo -u postgres psql <<EOF
ALTER USER postgres WITH PASSWORD '${DB_PASS}';
SELECT 'CREATE DATABASE ${DB_NAME}' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '${DB_NAME}')\gexec
EOF

# Update pg_hba.conf to allow password auth for localhost
PG_HBA=$(sudo -u postgres psql -t -P format=unaligned -c "SHOW hba_file;")
if ! grep -q "host.*all.*all.*127.0.0.1/32.*md5" "$PG_HBA" 2>/dev/null; then
    echo "[*] Updating pg_hba.conf for password authentication..."
    # Backup original
    cp "$PG_HBA" "${PG_HBA}.bak"
    # Add md5 auth for localhost before any existing rules
    sed -i '/^# IPv4 local connections:/a host    all             all             127.0.0.1/32            md5' "$PG_HBA"
    # Reload PostgreSQL to apply changes
    systemctl reload postgresql
fi

echo "[*] Testing connection..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d "${DB_NAME}" -c "SELECT 1;" > /dev/null 2>&1 && \
    echo "[+] PostgreSQL setup complete!" || \
    echo "[-] Connection test failed. You may need to restart PostgreSQL: sudo systemctl restart postgresql"

echo ""
echo "Connection details:"
echo "  Host:     localhost"
echo "  Port:     5432"
echo "  Database: ${DB_NAME}"
echo "  User:     ${DB_USER}"
echo "  Password: ${DB_PASS}"
