use tokio_postgres::NoTls;

#[tokio::main]
async fn main() {
    let url = std::env::var("PG_DSN").unwrap_or_else(|_| "postgres://meta:metapass@postgres:5432/meta".into());
    let (mut client, _) = match tokio_postgres::connect(&url, NoTls).await {
        Ok(c) => c,
        Err(e) => { eprintln!("connect fail: {}", e); return; }
    };
    println!("connect OK");
    match client.batch_execute("SELECT 1").await {
        Ok(_) => println!("select 1 OK"),
        Err(e) => { eprintln!("select 1 fail: {}", e); return; }
    }

    let name = "Mouse".to_string();
    let category = "acc".to_string();
    let slug = "m".to_string();
    let price = 35.0f64;
    let tenant = "default".to_string();

    println!("A) 5-param &[&String,&String,&String,&f64,&String]:");
    match client.query_one(
        "INSERT INTO shop_products (name, category, slug, price, tenant_id) VALUES ($1,$2,$3,$4::numeric,$5) RETURNING id",
        &[&name, &category, &slug, &price, &tenant],
    ).await {
        Ok(r) => println!("   OK id={}", r.get::<_,i64>(0)),
        Err(e) => println!("   FAIL {}", e),
    }

    println!("B) same but price via (f64) cast expr:");
    match client.query_one(
        "INSERT INTO shop_products (name, category, slug, price, tenant_id) VALUES ($1,$2,$3,$4::numeric,$5) RETURNING id",
        &[&name, &category, &slug, &(price as f64), &tenant],
    ).await {
        Ok(r) => println!("   OK id={}", r.get::<_,i64>(0)),
        Err(e) => println!("   FAIL {}", e),
    }

    println!("C) 4-param (no price):");
    match client.query_one(
        "INSERT INTO shop_products (name, category, slug, tenant_id) VALUES ($1,$2,$3,$4) RETURNING id",
        &[&name, &category, &slug, &tenant],
    ).await {
        Ok(r) => println!("   OK id={}", r.get::<_,i64>(0)),
        Err(e) => println!("   FAIL {}", e),
    }

    println!("D) 5-param but price as TEXT with ::numeric:");
    let price_s = "35".to_string();
    match client.query_one(
        "INSERT INTO shop_products (name, category, slug, price, tenant_id) VALUES ($1,$2,$3,$4::numeric,$5) RETURNING id",
        &[&name, &category, &slug, &price_s, &tenant],
    ).await {
        Ok(r) => println!("   OK id={}", r.get::<_,i64>(0)),
        Err(e) => println!("   FAIL {}", e),
    }

    let _ = client;
}
