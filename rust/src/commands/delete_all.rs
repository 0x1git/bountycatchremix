use anyhow::Result;
use deadpool_postgres::Pool;
use std::io::{self, Write};

pub async fn run(pool: &Pool, confirm: bool, silent: bool) -> Result<()> {
    if !confirm {
        print!("Are you sure you want to delete ALL domains from the database? (y/N): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            if !silent {
                eprintln!("Delete operation cancelled");
            }
            return Ok(());
        }
    }

    let client = pool.get().await?;

    // Check if table has data
    let row = client
        .query_one("SELECT EXISTS(SELECT 1 FROM domains LIMIT 1)", &[])
        .await?;
    let has_data: bool = row.get(0);

    if !has_data {
        if !silent {
            eprintln!("No domains existed in database");
        }
        return Ok(());
    }

    client.execute("TRUNCATE TABLE domains", &[]).await?;
    println!("All domains deleted successfully");

    Ok(())
}
