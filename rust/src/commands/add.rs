use anyhow::Result;
use deadpool_postgres::Pool;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::fs::File;
use std::time::Instant;
use tokio_postgres::types::ToSql;

use crate::domain::is_valid_domain;

const BATCH_SIZE: usize = 100_000;
const COPY_CHUNK_SIZE: usize = 5_000_000;

pub async fn run(
    pool: &Pool,
    file: Option<PathBuf>,
    validate: bool,
    silent: bool,
) -> Result<()> {
    let start = Instant::now();

    run_fast(pool, file, validate, silent).await?;

    if !silent {
        eprintln!("Completed in {:.1}s", start.elapsed().as_secs_f64());
    }

    Ok(())
}

async fn run_fast(
    pool: &Pool,
    file: Option<PathBuf>,
    validate: bool,
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

    let reader: Box<dyn BufRead> = match file {
        Some(path) => Box::new(BufReader::with_capacity(1024 * 1024, File::open(path)?)),
        None => Box::new(BufReader::with_capacity(1024 * 1024, io::stdin().lock())),
    };

    let mut total = 0u64;
    let mut invalid = 0u64;
    let mut buffer = Vec::with_capacity(COPY_CHUNK_SIZE);

    // Build COPY data
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

        buffer.push(domain.to_string());

        if buffer.len() >= COPY_CHUNK_SIZE {
            copy_domains(&client, &buffer).await?;
            buffer.clear();
        }
    }

    // Final chunk
    if !buffer.is_empty() {
        copy_domains(&client, &buffer).await?;
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

async fn run_batch(
    pool: &Pool,
    file: Option<PathBuf>,
    validate: bool,
    silent: bool,
) -> Result<()> {
    let client = pool.get().await?;
    let start = Instant::now();

    let reader: Box<dyn BufRead> = match file {
        Some(path) => Box::new(BufReader::with_capacity(512 * 1024, File::open(path)?)),
        None => Box::new(BufReader::with_capacity(512 * 1024, io::stdin().lock())),
    };

    let mut total = 0u64;
    let mut new_count = 0u64;
    let mut invalid = 0u64;
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);

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

        batch.push(domain.to_string());

        if batch.len() >= BATCH_SIZE {
            new_count += insert_batch(&client, &batch).await?;
            batch.clear();
        }
    }

    if !batch.is_empty() {
        new_count += insert_batch(&client, &batch).await?;
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
