use anyhow::Result;
use deadpool_postgres::Pool;
use regex::Regex;

pub async fn run(
    pool: &Pool,
    match_filter: Option<String>,
    regex_filter: Option<String>,
    silent: bool,
) -> Result<()> {
    let client = pool.get().await?;
    let _ = silent; // suppress unused warning

    let count: i64 = if match_filter.is_some() || regex_filter.is_some() {
        let regex = if let Some(pattern) = &regex_filter {
            Some(Regex::new(pattern)?)
        } else {
            None
        };

        let rows = client.query("SELECT domain FROM domains", &[]).await?;
        let mut count = 0i64;

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

            count += 1;
        }
        count
    } else {
        // Fast direct COUNT(*) when no filters
        let row = client.query_one("SELECT COUNT(*) FROM domains", &[]).await?;
        row.get(0)
    };

    println!("{}", count);

    Ok(())
}
