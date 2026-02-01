use anyhow::Result;
use deadpool_postgres::Pool;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::fs::File;
use std::time::Instant;
use tokio_postgres::types::ToSql;

use crate::domain::is_valid_domain;

const BATCH_SIZE: usize = 10_000;
const COPY_CHUNK_SIZE: usize = 5_000_000;
// Threshold: use COPY+rebuild for large imports, INSERT for small ones
const BULK_THRESHOLD: usize = 100_000;

pub async fn run(
    pool: &Pool,
    file: Option<PathBuf>,
    validate: bool,
    silent: bool,
) -> Result<()> {
    let start = Instant::now();

    // First, read and validate all domains into memory
    let reader: Box<dyn BufRead> = match &file {
        Some(path) => Box::new(BufReader::with_capacity(1024 * 1024, File::open(path)?)),
        None => Box::new(BufReader::with_capacity(1024 * 1024, io::stdin().lock())),
    };

    let mut domains: Vec<String> = Vec::new();
    let mut total = 0u64;
    let mut invalid = 0u64;

    for line in reader.lines() {
        let line = line?;
        let domain = line.trim();
        if domain.is_empty() {
            continue;
        }

        total += 1;

        if validate && !is_valid_domain(domain) {
            invalid += 1;
            continue;
        }

        domains.push(domain.to_string());
    }

    // Choose strategy based on batch size
    if domains.len() >= BULK_THRESHOLD {
        if !silent {
            eprintln!("Adding {} domains (bulk COPY mode)...", domains.len());
        }
        run_bulk_copy(pool, domains, total, invalid, silent).await?;
    } else {
        if !silent && domains.len() > 0 {
            eprintln!("Adding {} domains...", domains.len());
        }
        run_insert(pool, domains, total, invalid, silent).await?;
    }

    if !silent {
        eprintln!("Completed in {:.1}s", start.elapsed().as_secs_f64());
    }

    Ok(())
}

/// Fast INSERT with ON CONFLICT for small batches (< 100K domains)
async fn run_insert(
    pool: &Pool,
    domains: Vec<String>,
    total: u64,
    invalid: u64,
    silent: bool,
) -> Result<()> {
    let client = pool.get().await?;
    let start = Instant::now();

    let mut new_count = 0u64;

    // Process in batches
    for chunk in domains.chunks(BATCH_SIZE) {
        new_count += insert_batch(&client, chunk).await?;
    }

    let valid_count = total - invalid;
    let duplicate_count = valid_count - new_count;

    if !silent {
        let pct = if valid_count > 0 {
            (duplicate_count as f64 / valid_count as f64) * 100.0
        } else {
            0.0
        };
        eprintln!(
            "Processed {} domains: {} new, {} duplicates ({:.2}%) in {:.1}s",
            total, new_count, duplicate_count, pct, start.elapsed().as_secs_f64()
        );
        if invalid > 0 {
            eprintln!("Skipped {} invalid domains", invalid);
        }
    }

    Ok(())
}

/// Bulk COPY with index rebuild for large imports (>= 100K domains)
async fn run_bulk_copy(
    pool: &Pool,
    domains: Vec<String>,
    total: u64,
    invalid: u64,
    silent: bool,
) -> Result<()> {
    let client = pool.get().await?;
    let start = Instant::now();
    
    // Get initial count
    let row = client.query_one("SELECT COUNT(*) FROM domains", &[]).await?;
    let before_count: i64 = row.get(0);

    if !silent {
        eprintln!("Processing domains with COPY (streaming)...");
    }

    // Drop indexes for fast insert
    client.execute("ALTER TABLE domains DROP CONSTRAINT IF EXISTS domains_pkey CASCADE", &[]).await?;
    client.execute("DROP INDEX IF EXISTS idx_domains_domain", &[]).await?;

    // Optimize session
    client.execute("SET LOCAL synchronous_commit = OFF", &[]).await?;
    client.execute("SET LOCAL work_mem = '256MB'", &[]).await?;
    client.execute("SET LOCAL maintenance_work_mem = '512MB'", &[]).await?;

    // Insert in chunks
    for chunk in domains.chunks(COPY_CHUNK_SIZE) {
        copy_domains(&client, chunk).await?;
    }

    // Deduplicate
    if !silent {
        eprintln!("Deduplicating...");
    }
    client.execute(
        "DELETE FROM domains a USING domains b WHERE a.ctid < b.ctid AND a.domain = b.domain",
        &[],
    ).await?;

    // Rebuild indexes
    if !silent {
        eprintln!("Rebuilding indexes...");
    }
    client.execute("ALTER TABLE domains ADD PRIMARY KEY (domain)", &[]).await?;
    client.execute("CREATE INDEX idx_domains_domain ON domains (domain text_pattern_ops)", &[]).await?;

    // Get final count
    let row = client.query_one("SELECT COUNT(*) FROM domains", &[]).await?;
    let after_count: i64 = row.get(0);
    let new_count = after_count - before_count;
    let valid_count = total - invalid;
    let duplicate_count = valid_count as i64 - new_count;

    if !silent {
        let pct = if valid_count > 0 {
            (duplicate_count as f64 / valid_count as f64) * 100.0
        } else {
            0.0
        };
        eprintln!(
            "Processed {} domains: {} new, {} duplicates ({:.2}%) in {:.1}s",
            total, new_count, duplicate_count, pct, start.elapsed().as_secs_f64()
        );
        if invalid > 0 {
            eprintln!("Skipped {} invalid domains", invalid);
        }
    }

    Ok(())
}

async fn copy_domains(client: &deadpool_postgres::Client, domains: &[String]) -> Result<()> {
    // Use text-based COPY (more compatible than binary)
    let sink = client
        .copy_in("COPY domains (domain) FROM STDIN WITH (FORMAT text)")
        .await?;
    
    // Build text data
    let mut data = String::with_capacity(domains.len() * 50);
    for domain in domains {
        data.push_str(domain);
        data.push('\n');
    }
    
    use futures_util::SinkExt;
    let mut sink = std::pin::pin!(sink);
    sink.send(bytes::Bytes::from(data)).await?;
    sink.close().await?;
    
    Ok(())
}

async fn insert_batch(client: &deadpool_postgres::Client, domains: &[String]) -> Result<u64> {
    if domains.is_empty() {
        return Ok(0);
    }

    // Build parameterized query
    let mut query = String::from("INSERT INTO domains (domain) VALUES ");
    let mut params: Vec<&(dyn ToSql + Sync)> = Vec::with_capacity(domains.len());

    for (i, domain) in domains.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        query.push_str(&format!("(${})", i + 1));
        params.push(domain);
    }
    query.push_str(" ON CONFLICT DO NOTHING");

    let result = client.execute(&query, &params).await?;
    Ok(result)
}
