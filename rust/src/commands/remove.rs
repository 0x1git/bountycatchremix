use anyhow::Result;
use deadpool_postgres::Pool;
use regex::Regex;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::time::Instant;
use tokio_postgres::types::ToSql;

const BATCH_SIZE: usize = 10_000;

pub async fn run(
    pool: &Pool,
    file: Option<PathBuf>,
    domain: Option<String>,
    match_filter: Option<String>,
    regex_filter: Option<String>,
    silent: bool,
) -> Result<()> {
    let client = pool.get().await?;

    if let Some(d) = domain {
        // Single domain removal
        let result = client
            .execute("DELETE FROM domains WHERE domain = $1", &[&d])
            .await?;
        if result > 0 {
            println!("Domain '{}' removed from database", d);
        } else if !silent {
            eprintln!("Domain '{}' not found in database", d);
        }
        return Ok(());
    }

    if match_filter.is_some() || regex_filter.is_some() {
        // Filter-based removal
        let regex = if let Some(pattern) = &regex_filter {
            Some(Regex::new(pattern)?)
        } else {
            None
        };

        let rows = client.query("SELECT domain FROM domains", &[]).await?;
        let mut to_remove: Vec<String> = Vec::new();

        for row in rows {
            let d: String = row.get(0);

            if let Some(ref m) = match_filter {
                if !d.contains(m.as_str()) {
                    continue;
                }
            }

            if let Some(ref re) = regex {
                if !re.is_match(&d) {
                    continue;
                }
            }

            to_remove.push(d);
        }

        let removed = remove_batch(&client, &to_remove).await?;
        if !silent {
            eprintln!("Removed {} domains using filter", removed);
        }
        return Ok(());
    }

    // File/stdin-based removal - use fast COPY by default
    let start = Instant::now();

    run_fast_remove(pool, file, silent).await?;

    if !silent {
        eprintln!("Completed in {:.1}s", start.elapsed().as_secs_f64());
    }

    Ok(())
}

async fn run_fast_remove(pool: &Pool, file: Option<PathBuf>, silent: bool) -> Result<()> {
    let client = pool.get().await?;
    let start = Instant::now();

    // Create temp table
    client
        .execute(
            "CREATE TEMP TABLE temp_remove (domain TEXT) ON COMMIT DROP",
            &[],
        )
        .await?;

    let reader: Box<dyn BufRead> = match file {
        Some(path) => Box::new(BufReader::with_capacity(512 * 1024, File::open(path)?)),
        None => Box::new(BufReader::with_capacity(512 * 1024, io::stdin().lock())),
    };

    let mut domains: Vec<String> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let domain = line.trim();
        if !domain.is_empty() {
            domains.push(domain.to_string());
        }
    }

    // Use COPY to insert into temp table
    if !domains.is_empty() {
        let sink = client
            .copy_in("COPY temp_remove (domain) FROM STDIN")
            .await?;
        
        let writer = tokio_postgres::binary_copy::BinaryCopyInWriter::new(
            sink,
            &[tokio_postgres::types::Type::TEXT],
        );
        
        tokio::pin!(writer);
        
        for domain in &domains {
            writer.as_mut().write(&[domain]).await?;
        }
        
        writer.finish().await?;

        // Delete matching domains
        let result = client
            .execute(
                "DELETE FROM domains WHERE domain IN (SELECT domain FROM temp_remove)",
                &[],
            )
            .await?;

        if !silent {
            eprintln!(
                "Removed {} domains in {:.1}s (fast COPY)",
                result,
                start.elapsed().as_secs_f64()
            );
        }
    }

    Ok(())
}

async fn run_batch_remove(
    client: &deadpool_postgres::Client,
    file: Option<PathBuf>,
    silent: bool,
) -> Result<()> {
    let reader: Box<dyn BufRead> = match file {
        Some(path) => Box::new(BufReader::with_capacity(512 * 1024, File::open(path)?)),
        None => Box::new(BufReader::with_capacity(512 * 1024, io::stdin().lock())),
    };

    let mut total = 0u64;
    let mut removed = 0u64;
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);

    for line in reader.lines() {
        let line = line?;
        let domain = line.trim();
        if domain.is_empty() {
            continue;
        }

        total += 1;
        batch.push(domain.to_string());

        if batch.len() >= BATCH_SIZE {
            removed += remove_batch(client, &batch).await?;
            batch.clear();
        }
    }

    if !batch.is_empty() {
        removed += remove_batch(client, &batch).await?;
    }

    if !silent {
        eprintln!(
            "Processed {} domains: {} removed, {} not found",
            total,
            removed,
            total - removed
        );
    }

    Ok(())
}

async fn remove_batch(client: &deadpool_postgres::Client, domains: &[String]) -> Result<u64> {
    if domains.is_empty() {
        return Ok(0);
    }

    // Build parameterized query
    let placeholders: Vec<String> = (1..=domains.len()).map(|i| format!("${}", i)).collect();
    let query = format!(
        "DELETE FROM domains WHERE domain IN ({})",
        placeholders.join(", ")
    );

    let params: Vec<&(dyn ToSql + Sync)> = domains.iter().map(|d| d as &(dyn ToSql + Sync)).collect();
    let result = client.execute(&query, &params).await?;
    Ok(result)
}
