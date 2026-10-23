//! One real call, only when asked for: `MNEMORPH_LIVE=1 ANTHROPIC_API_KEY=… cargo nextest run -p
//! mnemorph-provider --run-ignored all live`. It spends a few hundred tokens of the key's budget.

use std::sync::Arc;

use mnemorph_provider::{
    Anthropic, Block, HttpTransport, Message, Policy, Request, Role, StopReason, anthropic_prices,
};

#[tokio::test]
#[ignore = "spends tokens: run with MNEMORPH_LIVE=1 and ANTHROPIC_API_KEY"]
async fn one_real_answer() {
    if std::env::var("MNEMORPH_LIVE").as_deref() != Ok("1") {
        return;
    }
    let key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY");
    let model = std::env::var("MNEMORPH_LIVE_MODEL").unwrap_or_else(|_| "claude-opus-5-5".into());
    let policy = Policy::default();
    let transport = Arc::new(HttpTransport::new(policy.connect).expect("tls"));
    let client = Anthropic::new(transport, "https://api.anthropic.com", key, policy);
    let request = Request {
        model: model.clone(),
        system: vec![],
        tools: vec![],
        messages: vec![Message {
            role: Role::User,
            content: vec![Block::Text {
                text: "Reply with the single word: ready".into(),
                cache: false,
            }],
        }],
        max_tokens: 1024,
        effort: None,
    };
    let mut streamed = String::new();
    let mut on = |e: mnemorph_provider::Event| {
        if let mnemorph_provider::Event::Text(t) = e {
            streamed.push_str(&t);
        }
    };
    let r = client
        .complete(&request, &mut on)
        .await
        .expect("a live answer");
    assert_eq!(r.stop, StopReason::EndTurn);
    assert!(streamed.to_lowercase().contains("ready"), "{streamed}");
    assert!(r.usage.input > 0 && r.usage.output > 0);
    if let Some(price) = anthropic_prices().unwrap().get(&model) {
        assert!(price.usd(r.usage) > 0.0);
    }
}
