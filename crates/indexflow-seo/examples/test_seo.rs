// crates/indexflow-seo/examples/test_seo.rs
use indexflow_seo::SeoProbeClient;
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_url = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://www.inkvilion.com/en".to_string());

    println!("==================================================");
    println!("🛡️  正在执行工业级技术 SEO 与富媒体深度质检: {}", target_url);
    println!("==================================================");

    let prober = SeoProbeClient::new(
        "Mozilla/5.0 (compatible; IndexFlowBot/1.0; +https://www.indexflowing.com)",
        Duration::from_secs(15),
    )?;

    let res = prober.check_url(&target_url).await;

    // 1. 网络层协议与重定向判定
    println!("【1. 网络协议层与门禁裁决】");
    println!("  HTTP Status   : {:?}", res.http_status);
    println!("  Response Time : {:?} ms", res.response_time_ms);
    println!("  Payload Size  : {:?} Bytes", res.payload_bytes);
    if let Some(ref loc) = res.redirect_location {
        println!("  重定向 Location: \x1b[33m{}\x1b[0m", loc);
    }
    if !res.http_links.is_empty() {
        println!("  HTTP Link 标头: (共 {} 条)", res.http_links.len());
        for link in &res.http_links {
            println!("    - <{}> rel=\"{}\" hreflang={:?}", link.uri, link.rel, link.hreflang);
        }
    }
    if res.passed {
        println!("  门禁裁决      : \x1b[32m✅ PASS (准许提交搜索引擎)\x1b[0m");
    } else {
        println!("  门禁裁决      : \x1b[31m❌ FAIL (拦截原因: {:?})\x1b[0m", res.block_reason);
    }
    println!();

    // 2. 核心技术 SEO 标签
    println!("【2. 核心技术 SEO 标签】");
    println!("  Page Title    : {}", res.page_title.as_deref().unwrap_or("<MISSING>"));
    println!("  Meta Desc     : {}", res.meta_description.as_deref().unwrap_or("<MISSING>"));
    println!("  H1 Content    : {} (总数: {})", res.h1_content.as_deref().unwrap_or("<MISSING>"), res.h1_count);
    println!("  Canonical URL : {} (有效一致: {})", res.canonical_url.as_deref().unwrap_or("<MISSING>"), res.has_canonical);
    println!("  Noindex 指令  : {}", res.has_noindex);
    println!("  Nofollow 指令 : {}", res.has_nofollow);
    if let Some(ref d) = res.robots_directive {
        println!("  Robots 指令   : {}", d);
    }
    if !res.hreflang.is_empty() {
        println!("  Hreflang ({})  : {:?}", res.hreflang.len(), res.hreflang);
    }
    println!();

    // 3. 结构化数据与 VideoObject 自环深度审计
    println!("【3. 结构化数据与 VideoObject 语义审计】");
    let schema_types = res.schema_types();
    if schema_types.is_empty() {
        println!("  JSON-LD Schemas: <未检测到结构化数据>");
    } else {
        println!("  JSON-LD Schemas: \x1b[36m{:?}\x1b[0m (共 {} 个 Block)", schema_types, res.json_ld.len());
    }
    if !res.video_objects.is_empty() {
        println!("  VideoObject ({}) :", res.video_objects.len());
        for (i, v) in res.video_objects.iter().enumerate() {
            println!("    [#{}] Name: {}", i, v.name.as_deref().unwrap_or("<无标题>"));
            println!("         embedUrl   : {:?}", v.embed_url);
            println!("         contentUrl : {:?}", v.content_url);
        }
    }
    println!("  OG Title      : {:?}", res.opengraph.title);
    println!("  OG Type       : {:?}", res.opengraph.og_type);
    println!("  OG Video      : {:?}", res.opengraph.video);
    println!("  Twitter Card  : {:?}", res.twitter_card.card);
    println!();

    // 4. AI 搜索引擎爬虫嗅探
    println!("【4. AI 爬虫探测策略 (GEO)】");
    println!("  GPTBot (OpenAI)       Blocked : {}", res.ai_directives.gptbot_blocked);
    println!("  PerplexityBot         Blocked : {}", res.ai_directives.perplexity_blocked);
    println!("  ClaudeBot (Anthropic) Blocked : {}", res.ai_directives.claudebot_blocked);
    println!("  Google-Extended       Blocked : {}", res.ai_directives.google_extended_blocked);

    // 5. 软性优化建议清单
    if !res.warnings.is_empty() {
        println!();
        println!("【5. 优化建议与工程警示清单 ({})】", res.warnings.len());
        for w in &res.warnings {
            println!("  \x1b[33m⚠️  {}\x1b[0m", w);
        }
    }
    println!("==================================================");

    Ok(())
}