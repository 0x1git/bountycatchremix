use anyhow::Result;
use deadpool_postgres::Pool;
use futures_util::StreamExt;
use regex::Regex;
use std::io::{self, Write};
use std::pin::pin;

pub async fn run(
    pool: &Pool,
    match_filter: Option<String>,
    regex_filter: Option<String>,
    sort: bool,
    silent: bool,
) -> Result<()> {
    let client = pool.get().await?;
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Use fast COPY when no filters are applied
    if match_filter.is_none() && regex_filter.is_none() && !sort {
        let reader = client
            .copy_out("COPY domains (domain) TO STDOUT")
            .await?;
        
        let mut pinned = pin!(reader);
        while let Some(chunk) = pinned.next().await {
            let data = chunk?;
            handle.write_all(&data)?;
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
        let mut found_any = false;

        for row in rows {
            let domain: &str = row.get(0);

            if let Some(ref m) = match_filter {
                if !domain.contains(m.as_str()) {
                    continue;
                }
            }

            if let Some(ref re) = regex {
                if !re.is_match(domain) {
                    continue;
                }
            }

            found_any = true;
            writeln!(handle, "{}", domain)?;
        }

        if !found_any && !silent {
            eprintln!("No domains found in database");
        }
    }

    Ok(())
}
