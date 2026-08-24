// crates/indexflow-sitemap/examples/test_sitemap.rs
use indexflow_sitemap::SitemapFetcher;
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 默认测试你的站点，或者接收命令行参数
    let target_url = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://www.inkvilion.com/sitemap.xml".to_string());

    println!("==================================================");
    println!("🔍 正在测试解析 Sitemap: {}", target_url);
    println!("==================================================");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (compatible; IndexFlowBot/1.0)")
        .build()?;

    let fetcher = SitemapFetcher::new(client);

    let start = std::time::Instant::now();
    let (is_index, entries) = fetcher.expand_all(&target_url, 3).await?;
    let elapsed = start.elapsed();

    println!("✅ 解析成功！耗时: {:?}", elapsed);
    println!("📑 是否为 SitemapIndex 索引树: {}", is_index);
    println!("📊 提取到的 URL 页面总数: {}", entries.len());
    println!("--------------------------------------------------");

    // 打印前 5 条详细数据
    for (i, entry) in entries.iter().take(5).enumerate() {
        println!("【#{}】URL: {}", i + 1, entry.loc);
        if let Some(lastmod) = entry.lastmod {
            println!("     Lastmod: {}", lastmod);
        }
        if let Some(priority) = entry.priority {
            println!("     Priority: {}", priority);
        }
        if let Some(changefreq) = entry.changefreq {
            println!("     ChangeFreq: {:?}", changefreq);
        }
        if !entry.hreflangs.is_empty() {
            println!("     Hreflangs: {:?}", entry.hreflangs);
        }
        if !entry.images.is_empty() {
            println!("     Images ({}): {:?}", entry.images.len(), entry.images);
        }
        if !entry.videos.is_empty() {
            println!("     Videos ({}): {:?}", entry.videos.len(), entry.videos);
        }
        if let Some(ref news) = entry.news {
            println!("     News: {:?}", news);
        }
        println!();
    }

    if entries.len() > 5 {
        println!("... 剩余 {} 条已成功解析省略显示", entries.len() - 5);
    }

    Ok(())
}