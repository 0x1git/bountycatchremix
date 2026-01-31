use anyhow::Result;
use chrono::Utc;
use deadpool_postgres::Pool;
use futures_util::StreamExt;
use regex::Regex;
use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::pin::pin;

#[derive(Serialize)]
struct ExportData {
    domain_count: usize,
    exported_at: String,
    domains: Vec<String>,
}

pub async fn run(
    pool: &Pool,
    file: PathBuf,
    format: String,
    match_filter: Option<String>,
    regex_filter: Option<String>,
    sort: bool,
    silent: bool,
) -> Result<()> {
    let client = pool.get().await?;

    // Use fast COPY when no filters and text format
    if match_filter.is_none() && regex_filter.is_none() && !sort && format != "json" {
        let output = File::create(&file)?;
        let mut writer = BufWriter::with_capacity(1024 * 1024, output);
        
        let reader = client
            .copy_out("COPY domains (domain) TO STDOUT")
            .await?;
        
        let mut pinned = pin!(reader);
        while let Some(chunk) = pinned.next().await {
            let data = chunk?;
            writer.write_all(&data)?;
        }
        writer.flush()?;

        // Get count for logging
        let row = client.query_one("SELECT COUNT(*) FROM domains", &[]).await?;
        let count: i64 = row.get(0);

        if !silent {
            eprintln!("Exported {} domains to {:?}", count, file);
        }
    } else {
        let regex = if let Some(pattern) = &regex_filter {
            Some(Regex::new(pattern)?)
        } else {
            None
        };

        let query = if sort {
            "SELECT domain FROM domains ORDER BY domain"
        } else {
            "SELECT domain FROM domains"
        };

        let rows = client.query(query, &[]).await?;
        let mut domains: Vec<String> = Vec::new();

        for row in rows {
            let domain: String = row.get(0);

            if let Some(ref m) = match_filter {
                if !domain.contains(m.as_str()) {
                    continue;
                }
            }

            if let Some(ref re) = regex {
                if !re.is_match(&domain) {
                    continue;
                }
            }

            domains.push(domain);
        }

        let count = domains.len();

        if format == "json" {
            let export_data = ExportData {
                domain_count: count,
                exported_at: Utc::now().to_rfc3339(),
                domains,
            };
            let output = File::create(&file)?;
            serde_json::to_writer_pretty(output, &export_data)?;
        } else {
            let output = File::create(&file)?;
            let mut writer = BufWriter::with_capacity(1024 * 1024, output);
            for domain in &domains {
                writeln!(writer, "{}", domain)?;
            }
            writer.flush()?;
        }

        if !silent {
            eprintln!("Exported {} domains to {:?} ({} format)", count, file, format);
        }
    }

    Ok(())
}
