//! End-to-end API tests against a real Postgres.
//!
//! Set `TEST_DATABASE_URL` to a server the test may create databases on, e.g.
//! `postgresql://postgres:postgres@localhost:5432/postgres`. Without it the test is skipped.

use std::sync::Arc;

use alloy_primitives::U256;
use arc_stablecoin_stream::api::{self, AppState};
use arc_stablecoin_stream::config::{Config, Contracts, Network, ReferenceRates};
use arc_stablecoin_stream::db;
use arc_stablecoin_stream::enrich::{NewFlow, NewLink};
use arc_stablecoin_stream::ingest::Progress;
use arc_stablecoin_stream::rates::LiveRates;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::{Connection, PgConnection, PgPool};
use tokio::sync::broadcast;
use tower::ServiceExt;

const ADMIN_KEY: &str = "tdl_test_admin";
const DEPOSITOR: &str = "0x631fdaafe9ff675bf7a122013d1555513363dcc7";
const SIGNER: &str = "0x10f48f71dee384425ac52ea3dbf5180a95d3f5ab";
const STRANGER: &str = "0x00000000000000000000000000000000000a11ce";

/// A fresh database per run, so tests never touch development data.
async fn fresh_database(server_url: &str) -> (PgPool, String, String) {
    let name = format!("tideline_test_{}", std::process::id());
    let mut admin = PgConnection::connect(server_url).await.unwrap();
    sqlx::query(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query(&format!("CREATE DATABASE {name}"))
        .execute(&mut admin)
        .await
        .unwrap();
    let (base, _) = server_url.rsplit_once('/').unwrap();
    let url = format!("{base}/{name}");
    (db::connect(&url).await.unwrap(), name, url)
}

fn flow(tx: &str, kind: &'static str, from: &str, to: &str, amount: Option<u64>) -> NewFlow {
    NewFlow {
        tx_hash: tx.to_string(),
        log_index: 0,
        block_number: 100,
        asset: "USDC",
        kind,
        from_address: from.to_string(),
        to_address: to.to_string(),
        amount: amount.map(U256::from),
        usd_value: amount.map(|a| a as f64 / 1e6),
        visibility: if amount.is_some() {
            "public"
        } else {
            "shielded"
        },
        src_domain: None,
        dst_domain: (kind == "gateway_outbound").then_some(26),
        details: json!({ "protocol": "test" }),
    }
}

async fn call(app: &Router, req: Request<Body>) -> (StatusCode, Value, String) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes).to_string();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        text,
    )
}

fn get(path: &str, key: Option<&str>) -> Request<Body> {
    let mut req = Request::builder().uri(path);
    if let Some(key) = key {
        req = req.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    req.body(Body::empty()).unwrap()
}

fn post(path: &str, key: Option<&str>, body: Value) -> Request<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(key) = key {
        req = req.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    req.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn attribution_licensing_and_disclosure_end_to_end() {
    let Ok(server_url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set: skipping database-backed API test");
        return;
    };
    let (pool, db_name, db_url) = fresh_database(&server_url).await;
    // A fresh database binds to the first network it starts on, and keeps it.
    db::bind_network(&pool, Network::Testnet).await.unwrap();
    db::bind_network(&pool, Network::Testnet).await.unwrap();
    let err = db::bind_network(&pool, Network::Mainnet).await.unwrap_err();
    assert!(err.to_string().contains("holds Arc Testnet data"), "{err}");
    db::seed_registry(&pool, Network::Testnet).await.unwrap();
    // Seeding twice must be a no-op, not a conflict.
    db::seed_registry(&pool, Network::Testnet).await.unwrap();

    let now = chrono::Utc::now().timestamp();
    let rows = vec![
        (
            flow(
                "0xaa",
                "gateway_outbound",
                DEPOSITOR,
                STRANGER,
                Some(2_000_000_000_000),
            ),
            now,
        ),
        (flow("0xbb", "transfer", SIGNER, STRANGER, None), now),
        (
            flow(
                "0xcc",
                "transfer",
                STRANGER,
                "0x00000000000000000000000000000000000000b0",
                Some(5_000_000),
            ),
            now,
        ),
    ];
    let mut tx = pool.begin().await.unwrap();
    let ids = db::insert_flows(&mut tx, &rows, 0).await.unwrap();
    assert_eq!(ids.len(), 3);
    // Re-ingesting the same range stores nothing new.
    assert!(db::insert_flows(&mut tx, &rows, 0)
        .await
        .unwrap()
        .is_empty());
    db::insert_links(
        &mut tx,
        &[NewLink {
            address_a: DEPOSITOR.into(),
            address_b: SIGNER.into(),
            link_type: "gateway_delegate",
            evidence_tx: "0xaa".into(),
        }],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let (outbound_id, shielded_id, public_id) = (ids[0], ids[1], ids[2]);

    let cfg = Arc::new(Config {
        network: Network::Testnet,
        database_url: db_url,
        rpc_urls: Vec::new(),
        port: 0,
        ingest_enabled: false,
        backfill_blocks: 0,
        confirmations: 0,
        max_chunk_blocks: 1,
        poll_interval_ms: 1000,
        rates: ReferenceRates {
            eurc_usd: 1.08,
            usyc_usd: 1.10,
        },
        live_rates: false,
        admin_key: Some(ADMIN_KEY.into()),
        confidential_transfer_topic: None,
        contracts: Contracts::arc_testnet(),
    });
    let (new_flows, _) = broadcast::channel(8);
    let rates = Arc::new(LiveRates::new(cfg.rates).unwrap());
    let app = api::app(AppState::new(
        pool.clone(),
        cfg,
        Arc::new(Progress::new()),
        new_flows,
        rates,
    ));

    // Public tier: flows are visible, decoded context is not.
    let (status, body, _) = call(&app, get("/v1/flows", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["flows"].as_array().unwrap().len(), 3);
    assert!(body["flows"][0]["details"].is_null());
    let (_, body, _) = call(&app, get("/v1/flows", Some(ADMIN_KEY))).await;
    assert_eq!(body["flows"][0]["details"]["protocol"], "test");

    // Licensed endpoints reject anonymous and unknown callers.
    let (status, body, _) = call(&app, get("/v1/compliance/report?entity=visa", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "license_required");
    let (status, _, _) = call(&app, get("/v1/flows", Some("tdl_nope"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // The registry lists attributed entities first, then the watchlist.
    let (status, body, _) = call(&app, get("/v1/entities", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["slug"], "circle");
    assert_eq!(body[0]["status"], "attributed");
    assert_eq!(body.as_array().unwrap().len(), 11);
    for path in ["/v1/overview", "/v1/license", "/v1/entities/circle"] {
        let (status, _, _) = call(&app, get(path, None)).await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }
    let (_, body, _) = call(&app, get("/v1/network", None)).await;
    assert_eq!(body["chain_id"], 5042002);
    assert_eq!(body["explorer_url"], "https://explorer.testnet.arc.io");
    // Only the seeded network's protocol contracts are labelled.
    let (_, body, _) = call(&app, get("/v1/entities/circle", None)).await;
    assert_eq!(body["addresses"].as_array().unwrap().len(), 13);

    // Nothing is attributed to Visa before curation.
    let (_, body, _) = call(&app, get("/v1/entities/visa", None)).await;
    assert_eq!(body["entity"]["status"], "watchlist");

    // Curation requires admin, and requires evidence.
    let attribution = json!({
        "entity_slug": "visa", "address": DEPOSITOR, "label": "Treasury",
        "confidence": 0.9, "source": "test", "evidence": { "note": "fixture" },
    });
    let (status, _, _) = call(&app, post("/v1/attributions", None, attribution.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let mut no_evidence = attribution.clone();
    no_evidence["evidence"] = json!({});
    let (status, _, _) = call(&app, post("/v1/attributions", Some(ADMIN_KEY), no_evidence)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body, _) =
        call(&app, post("/v1/attributions", Some(ADMIN_KEY), attribution)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body[0]["method"], "curated");

    // The Gateway signer inherits the entity through the on-chain link, at lower confidence.
    let (_, body, _) = call(
        &app,
        get(&format!("/v1/addresses/{SIGNER}"), Some(ADMIN_KEY)),
    )
    .await;
    assert_eq!(body["labels"][0]["method"], "heuristic");
    assert_eq!(body["labels"][0]["entity"]["slug"], "visa");
    assert!((body["labels"][0]["confidence"].as_f64().unwrap() - 0.675).abs() < 1e-4);
    assert_eq!(body["labels"][0]["evidence"]["evidence_tx"], "0xaa");
    assert_eq!(body["links"][0]["address"], DEPOSITOR);

    // History is relabelled: both Visa flows now resolve, narrative names the entity.
    let (_, body, _) = call(&app, get("/v1/flows?entity=visa", Some(ADMIN_KEY))).await;
    let flows = body["flows"].as_array().unwrap();
    assert_eq!(flows.len(), 2);
    let outbound = flows.iter().find(|f| f["id"] == outbound_id).unwrap();
    assert_eq!(outbound["from"]["entity"]["name"], "Visa");
    assert_eq!(outbound["attribution"], "partial");
    assert_eq!(
        outbound["narrative"],
        "$2.0M institutional USDC Gateway transfer out to Arc: Visa → 0x0000…11ce"
    );
    let shielded = flows.iter().find(|f| f["id"] == shielded_id).unwrap();
    assert_eq!(shielded["visibility"], "shielded");
    assert!(shielded["amount"].is_null());
    assert!(shielded["narrative"]
        .as_str()
        .unwrap()
        .starts_with("Amount-shielded USDC transfer: Visa"));

    // Coverage counts the shielded flow as attributed without inventing a value for it.
    let (_, body, _) = call(&app, get("/v1/coverage", None)).await;
    assert_eq!(body["flows_total"], 3);
    assert_eq!(body["flows_attributed"], 2);
    assert_eq!(body["volume_usd_total"], 2_000_005.0);
    assert_eq!(body["volume_usd_attributed"], 2_000_000.0);
    assert_eq!(body["institutional_volume_coverage_pct"], 100.0);
    let shielded_row = body["visibility"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["visibility"] == "shielded")
        .unwrap();
    assert_eq!(shielded_row["flows"], 1);
    assert_eq!(shielded_row["volume_usd"], 0.0);
    assert_eq!(body["top_unattributed"][0]["address"], STRANGER);
    // The window is declared as partly covered: history starts with the first stored flow.
    assert!(body["scope"]["history_start"].is_string());
    assert!(body["scope"]["history_seconds"].as_i64().unwrap() >= 0);
    assert_eq!(body["scope"]["window_complete"], false);
    assert_eq!(body["scope"]["shielding_live"], false);
    // Without a live source the configured rates are served, and labelled as static.
    let (_, body, _) = call(&app, get("/v1/overview", None)).await;
    assert_eq!(body["reference_rates"]["eurc_usd"], 1.08);
    assert_eq!(
        body["reference_rates"]["eurc_source"],
        "configured static rate"
    );
    assert!(body["reference_rates"]["eurc_as_of"].is_null());

    // Selective disclosure brings the shielded amount back into the numbers.
    let disclosure =
        |flow_id: i64| json!({ "flow_id": flow_id, "amount": "750000.5", "disclosed_by": "visa" });
    let (status, body, _) = call(
        &app,
        post("/v1/disclosures", Some(ADMIN_KEY), disclosure(shielded_id)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["visibility"], "disclosed");
    assert_eq!(body["amount"], "750000.5");
    assert_eq!(body["usd_value"], 750_000.5);
    let (status, body, _) = call(
        &app,
        post("/v1/disclosures", Some(ADMIN_KEY), disclosure(public_id)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "not_shielded");

    // A minted license key unlocks the compliance report, in JSON and CSV.
    let (status, body, _) = call(
        &app,
        post("/v1/keys", Some(ADMIN_KEY), json!({ "name": "desk" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let key = body["key"].as_str().unwrap().to_string();
    let (status, _, _) = call(&app, post("/v1/keys", Some(&key), json!({ "name": "x" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body, _) = call(&app, get("/v1/compliance/report?entity=visa", Some(&key))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["flow_count"], 2);
    assert_eq!(body["outflow_usd"], 2_750_000.5);
    assert_eq!(body["addresses"].as_array().unwrap().len(), 2);
    assert!(body["addresses"][0]["evidence"].is_object());
    let (status, _, csv) = call(
        &app,
        get("/v1/compliance/report?entity=visa&format=csv", Some(&key)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(csv.starts_with("flow_id,block_time,tx_hash"));
    assert_eq!(csv.lines().count(), 3);
    assert!(csv.contains(",Visa,curated,0.90,"));
    let usage: i64 = sqlx::query_scalar("SELECT COALESCE(sum(requests), 0)::bigint FROM api_usage")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        usage, 3,
        "every request made with the minted key is metered"
    );

    let (status, _, _) = call(&app, get("/v1/entities/nobody", None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(&app, get("/v1/addresses/0x1234", None)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    pool.close().await;
    let mut admin = PgConnection::connect(&server_url).await.unwrap();
    sqlx::query(&format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"))
        .execute(&mut admin)
        .await
        .unwrap();
}
