// crates/indexflow-seo/examples/test_seo.rs
use indexflow_seo::SeoProbeClient;
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 默认测试你的主页，或从命令行接收自定义 URL
    let target_url = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://www.inkvilion.com/en".to_string());

    println!("==================================================");
    println!("🛡️  正在执行技术 SEO 与 GEO 深度质检: {}", target_url);
    println!("==================================================");

    let prober = SeoProbeClient::new(
        "Mozilla/5.0 (compatible; IndexFlowBot/1.0; +https://www.indexflowing.com)",
        Duration::from_secs(15),
    )?;

    let res = prober.check_url(&target_url).await;

    // 1. 基础网络层与门禁裁决
    println!("【1. 网络层与最终决策】");
    println!("  HTTP Status   : {:?}", res.http_status);
    println!("  Response Time : {:?} ms", res.response_time_ms);
    println!("  Payload Size  : {:?} Bytes", res.payload_bytes);
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
    println!("  Canonical URL : {} (声明一致: {})", res.canonical_url.as_deref().unwrap_or("<MISSING>"), res.has_canonical);
    println!("  Noindex 指令  : {}", res.has_noindex);
    println!("  Nofollow 指令 : {}", res.has_nofollow);
    if let Some(ref d) = res.robots_directive {
        println!("  Robots 指令   : {}", d);
    }
    if !res.hreflang.is_empty() {
        println!("  Hreflang ({})  : {:?}", res.hreflang.len(), res.hreflang);
    }
    println!();

    // 3. GEO / AI 搜索引擎实体与结构化数据
    println!("【3. GEO & 结构化数据 (Schema.org / OpenGraph)】");
    let schema_types = res.schema_types();
    if schema_types.is_empty() {
        println!("  JSON-LD Schemas: <未检测到 JSON-LD 结构化数据>");
    } else {
        println!("  JSON-LD Schemas: \x1b[36m{:?}\x1b[0m (共 {} 个 Block)", schema_types, res.json_ld.len());
    }
    println!("  OG Title      : {:?}", res.opengraph.title);
    println!("  OG Type       : {:?}", res.opengraph.og_type);
    println!("  OG Image      : {:?}", res.opengraph.image);
    println!("  Twitter Card  : {:?}", res.twitter_card.card);
    println!();

    // 4. AI 爬虫拦截策略嗅探
    println!("【4. AI 爬虫探测策略】");
    println!("  GPTBot (OpenAI)       Blocked : {}", res.ai_directives.gptbot_blocked);
    println!("  PerplexityBot         Blocked : {}", res.ai_directives.perplexity_blocked);
    println!("  ClaudeBot (Anthropic) Blocked : {}", res.ai_directives.claudebot_blocked);
    println!("  Google-Extended       Blocked : {}", res.ai_directives.google_extended_blocked);
    println!("==================================================");

    Ok(())
}