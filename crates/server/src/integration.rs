use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, header},
};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;
fn app(db: sqlx::PgPool) -> Arc<App> {
    Arc::new(App {
        db,
        events: broadcast::channel(16).0,
        origin: "http://test.local".into(),
        key: [7; 32],
        private: true,
        secure: false,
        brand: "Test Aegis".into(),
    })
}
async fn request(
    a: &Arc<App>,
    method: &str,
    path: &str,
    body: Value,
    cookie: Option<&str>,
) -> (StatusCode, Value, Option<String>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/api/v1{path}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, "http://test.local");
    if let Some(c) = cookie {
        builder = builder.header(header::COOKIE, c);
    }
    let r = router(a.clone())
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = r.status();
    let cookie = r
        .headers()
        .get(header::SET_COOKIE)
        .map(|h| h.to_str().unwrap().split(';').next().unwrap().to_owned());
    let value = serde_json::from_slice(&to_bytes(r.into_body(), 262144).await.unwrap()).unwrap();
    (status, value, cookie)
}
async fn owner_session(a: &Arc<App>) -> String {
    let (status, _, cookie) = request(
        a,
        "POST",
        "/setup",
        json!({"email":"owner@example.test","password":"a-long-unique-test-password"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    cookie.unwrap()
}
#[sqlx::test(migrations = "../../migrations")]
async fn totp_requires_code_rejects_replay_and_disables_with_password(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let password = "a-long-unique-test-password";
    let (status, setup, _) = request(&a, "POST", "/auth/totp/setup", json!({"password":password}), Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(setup["uri"].as_str().unwrap().contains("algorithm=SHA256"));
    let encrypted: String = sqlx::query_scalar("SELECT totp_pending_secret FROM users WHERE email='owner@example.test'")
        .fetch_one(&a.db).await.unwrap();
    let secret = hex::decode(decrypt(&a, &encrypted).unwrap()).unwrap();
    let step = chrono::Utc::now().timestamp() / 30;
    let code = totp::code(&secret, step);
    let (status, _, _) = request(&a, "POST", "/auth/totp/enable", json!({"code":code}), Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(request(&a, "POST", "/auth/totp/setup", json!({"password":password}), Some(&cookie)).await.0, StatusCode::BAD_REQUEST);
    let credentials = json!({"email":"owner@example.test","password":password});
    assert_eq!(request(&a, "POST", "/auth/login", credentials.clone(), None).await.0, StatusCode::UNAUTHORIZED);
    let credentials = json!({"email":"owner@example.test","password":password,"totp_code":totp::code(&secret, step + 1)});
    assert_eq!(request(&a, "POST", "/auth/login", credentials.clone(), None).await.0, StatusCode::OK);
    assert_eq!(request(&a, "POST", "/auth/login", credentials, None).await.0, StatusCode::UNAUTHORIZED);
    let (status, _, _) = request(&a, "POST", "/auth/totp/disable", json!({"password":password,"code":totp::code(&secret, step + 1)}), Some(&cookie)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    sqlx::query("UPDATE users SET totp_last_step=$1 WHERE email='owner@example.test'")
        .bind(step - 1).execute(&a.db).await.unwrap();
    let (status, _, _) = request(&a, "POST", "/auth/totp/disable", json!({"password":password,"code":totp::code(&secret, step)}), Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(request(&a, "POST", "/auth/login", json!({"email":"owner@example.test","password":password}), None).await.0, StatusCode::OK);
}
#[sqlx::test(migrations = "../../migrations")]
async fn status_subscriber_jobs_use_only_published_text_and_tokens(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let page = Uuid::new_v4();
    let subscriber = Uuid::new_v4();
    let token = secret();
    sqlx::query("INSERT INTO status_pages(id,slug,title) VALUES($1,'public-test','Public Test')")
        .bind(page).execute(&a.db).await.unwrap();
    sqlx::query("INSERT INTO status_subscriptions(id,page_id,address_hash,address_cipher,confirm_hash) VALUES($1,$2,'address',$3,$4)")
        .bind(subscriber).bind(page).bind(encrypt(&a,"reader@example.test").unwrap()).bind(hash(&token)).execute(&a.db).await.unwrap();
    assert_eq!(request(&a,"POST","/public/subscriptions/confirm",json!({"id":subscriber,"token":token}),None).await.0,StatusCode::OK);
    assert_eq!(request(&a,"POST","/public/subscriptions/confirm",json!({"id":subscriber,"token":token}),None).await.0,StatusCode::BAD_REQUEST);
    assert_eq!(request(&a,"POST","/incidents",json!({"title":"Private incident","phase":"investigating","public_message":"Do not email yet","note":"private credential detail","published":false,"status_page_ids":[page]}),Some(&cookie)).await.0,StatusCode::OK);
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM status_subscription_deliveries").fetch_one(&a.db).await.unwrap();
    assert_eq!(queued,0);
    let (status, _, _) = request(&a,"POST","/incidents",json!({"title":"Public title","phase":"investigating","public_message":"Safe editorial update","note":"private credential detail","published":true,"status_page_ids":[page]}),Some(&cookie)).await;
    assert_eq!(status,StatusCode::OK);
    let cipher: String = sqlx::query_scalar("SELECT payload_cipher FROM status_subscription_deliveries WHERE subscription_id=$1")
        .bind(subscriber).fetch_one(&a.db).await.unwrap();
    assert!(!cipher.contains("private credential detail"));
    let payload: Value = serde_json::from_str(&decrypt(&a,&cipher).unwrap()).unwrap();
    assert_eq!(payload["reason"],"Safe editorial update");
    assert!(!payload.to_string().contains("private credential detail"));
    sqlx::query("UPDATE status_pages SET subscriptions_enabled=false WHERE id=$1")
        .bind(page).execute(&a.db).await.unwrap();
    subscriptions::deliver(&a).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT state FROM status_subscription_deliveries WHERE subscription_id=$1")
        .bind(subscriber).fetch_one(&a.db).await.unwrap();
    assert_eq!(state,"cancelled");
    assert_eq!(request(&a,"POST","/incidents",json!({"title":"Disabled page","phase":"investigating","public_message":"Another update","published":true,"status_page_ids":[page]}),Some(&cookie)).await.0,StatusCode::OK);
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM status_subscription_deliveries").fetch_one(&a.db).await.unwrap();
    assert_eq!(queued,1);
    let unsubscribe = subscriptions::test_unsubscribe_token(&a,subscriber);
    assert_eq!(request(&a,"POST","/public/subscriptions/unsubscribe",json!({"id":subscriber,"token":unsubscribe}),None).await.0,StatusCode::OK);
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM status_subscription_deliveries")
        .fetch_one(&a.db).await.unwrap();
    assert_eq!(remaining,0);
}
fn input() -> Value {
    json!({"name":"test website","kind":"http","target":"http://127.0.0.1:19191/health","interval_seconds":20,"timeout_seconds":5,"failure_threshold":2,"recovery_threshold":2,"public":true})
}
#[sqlx::test(migrations = "../../migrations")]
async fn auth_permissions_and_crud(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    assert_eq!(
        request(
            &a,
            "POST",
            "/setup",
            json!({"email":"other@example.test","password":"a-long-other-test-password"}),
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&a, "GET", "/monitors", json!(null), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (csrf, _, _) = request(&a, "POST", "/monitors", input(), Some(&cookie)).await;
    assert_eq!(csrf, StatusCode::OK);
    let r = router(a.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/monitors")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "https://evil.example")
                .body(Body::from(input().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert_eq!(request(&a,"POST","/users",json!({"email":"viewer@example.test","password":"another-secure-password","role":"viewer"}),Some(&cookie)).await.0,StatusCode::OK);
    let (status, _, viewer) = request(
        &a,
        "POST",
        "/auth/login",
        json!({"email":"viewer@example.test","password":"another-secure-password"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        request(&a, "POST", "/monitors", input(), viewer.as_deref())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (_, m, _) = request(&a, "POST", "/monitors", input(), Some(&cookie)).await;
    let id = m["id"].as_str().unwrap();
    assert_eq!(
        request(
            &a,
            "GET",
            &format!("/monitors/{id}"),
            json!(null),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &a,
            "DELETE",
            &format!("/monitors/{id}"),
            json!(null),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    request(&a, "POST", "/auth/logout", json!({}), Some(&cookie)).await;
    assert_eq!(
        request(&a, "GET", "/me", json!(null), Some(&cookie))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
#[sqlx::test(migrations = "../../migrations")]
async fn incident_hysteresis_dedup_public_redaction_and_maintenance(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let (_, created, _) = request(&a, "POST", "/monitors", input(), Some(&cookie)).await;
    let id: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    sqlx::query(
        "INSERT INTO notification_channels(id,name,provider,secret) VALUES($1,'test','webhook',$2)",
    )
    .bind(Uuid::new_v4())
    .bind(encrypt(&a, "http://127.0.0.1:19192").unwrap())
    .execute(&a.db)
    .await
    .unwrap();
    for _ in 0..4 {
        engine::record(
            &a,
            id,
            aegis_checks::Observation::from_error("Service failed"),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM incidents")
            .fetch_one(&a.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_deliveries")
            .fetch_one(&a.db)
            .await
            .unwrap(),
        1
    );
    let healthy = || aegis_checks::Observation {
        ok: true,
        latency_ms: 10.0,
        error: None,
        observed: json!({}),
    };
    engine::record(&a, id, healthy()).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM monitors WHERE id=$1")
            .bind(id)
            .fetch_one(&a.db)
            .await
            .unwrap(),
        "DOWN"
    );
    engine::record(&a, id, healthy()).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM incidents WHERE ended_at IS NOT NULL")
            .fetch_one(&a.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_deliveries")
            .fetch_one(&a.db)
            .await
            .unwrap(),
        2
    );
    request(
        &a,
        "POST",
        "/status-pages",
        json!({"title":"Public status","slug":"public","monitor_ids":[id]}),
        Some(&cookie),
    )
    .await;
    let (status, public, _) = request(&a, "GET", "/public/status/public", json!(null), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(public["monitors"].as_array().unwrap().len(), 1);
    assert!(!public.to_string().contains("127.0.0.1"));
    assert!(!public.to_string().contains("Service failed"));
    sqlx::query("INSERT INTO maintenance_windows(id,reason,starts_at,ends_at,monitor_ids) VALUES($1,'test',now()-interval '1 minute',now()+interval '1 hour',$2)").bind(Uuid::new_v4()).bind(vec![id]).execute(&a.db).await.unwrap();
    for _ in 0..3 {
        engine::record(&a, id, aegis_checks::Observation::from_error("Maintaining"))
            .await
            .unwrap();
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM incidents")
            .fetch_one(&a.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM monitors WHERE id=$1")
            .bind(id)
            .fetch_one(&a.db)
            .await
            .unwrap(),
        "MAINTENANCE"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn down_reminders_are_deduplicated_and_stop_after_recovery(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let (_, created, _) = request(&a, "POST", "/monitors", input(), Some(&cookie)).await;
    let monitor: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    let channel = Uuid::new_v4();
    let incident = Uuid::new_v4();
    sqlx::query("INSERT INTO notification_channels(id,name,provider,secret,reminder_seconds) VALUES($1,'reminders','webhook','encrypted',300)")
        .bind(channel).execute(&a.db).await.unwrap();
    sqlx::query("INSERT INTO incidents(id,monitor_id,title,started_at) VALUES($1,$2,'test',now()-interval '11 minutes')")
        .bind(incident).bind(monitor).execute(&a.db).await.unwrap();
    sqlx::query("INSERT INTO notification_deliveries(id,channel_id,incident_id,event,payload,state) VALUES($1,$2,$3,'DOWN','{}','sent')")
        .bind(Uuid::new_v4()).bind(channel).bind(incident).execute(&a.db).await.unwrap();
    engine::enqueue_reminders(&a).await.unwrap();
    engine::enqueue_reminders(&a).await.unwrap();
    let slots: Vec<i32> = sqlx::query_scalar("SELECT reminder_slot FROM notification_deliveries WHERE incident_id=$1 ORDER BY reminder_slot")
        .bind(incident).fetch_all(&a.db).await.unwrap();
    assert_eq!(slots, vec![0, 2]);
    sqlx::query("UPDATE incidents SET ended_at=now() WHERE id=$1")
        .bind(incident).execute(&a.db).await.unwrap();
    engine::enqueue_reminders(&a).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_deliveries WHERE incident_id=$1")
        .bind(incident).fetch_one(&a.db).await.unwrap();
    assert_eq!(count, 2);
}
#[sqlx::test(migrations = "../../migrations")]
async fn agent_auth_idempotency_rotation_and_ownership(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let (_, server, _) = request(
        &a,
        "POST",
        "/servers",
        json!({"name":"machine"}),
        Some(&cookie),
    )
    .await;
    let token = server["token"].as_str().unwrap();
    let id = server["id"].as_str().unwrap();
    let submission = Uuid::new_v4();
    let report = json!({"submission_id":submission,"version":"test","inventory":{"host":{"cpu_percent":5.0,"memory_percent":10.0}}});
    let agent_request = |token: String, report: Value| {
        let app = a.clone();
        async move { app_router_request(app, token, report).await }
    };
    assert_eq!(
        agent_request("wrong".into(), report.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        agent_request(token.into(), report.clone()).await.0,
        StatusCode::OK
    );
    assert_eq!(
        agent_request(token.into(), report.clone()).await.1["duplicate"],
        true
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM agent_submissions")
            .fetch_one(&a.db)
            .await
            .unwrap(),
        1
    );
    request(
        &a,
        "POST",
        &format!("/servers/{id}/rotate"),
        json!({}),
        Some(&cookie),
    )
    .await;
    assert_eq!(
        agent_request(token.into(), report).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (_, list, _) = request(&a, "GET", "/servers", json!(null), Some(&cookie)).await;
    assert!(!list.to_string().contains("token_hash"));
    assert!(!list.to_string().contains(token));
}
async fn app_router_request(a: Arc<App>, token: String, report: Value) -> (StatusCode, Value) {
    let r = router(a)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/agent/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(report.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = r.status();
    let value = serde_json::from_slice(&to_bytes(r.into_body(), 262144).await.unwrap()).unwrap();
    (status, value)
}
#[sqlx::test(migrations = "../../migrations")]
async fn time_weighted_uptime_respects_gaps(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let (_, created, _) = request(&a, "POST", "/monitors", input(), Some(&cookie)).await;
    let id: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    let empty = api::uptime_value(&a, id, 1).await.unwrap();
    assert!(empty["uptime"].is_null());
    sqlx::query("INSERT INTO checks(monitor_id,ok,state,latency_ms,created_at) VALUES($1,true,'UP',1,now()-interval '1 hour'),($1,false,'DOWN',1,now()-interval '59 minutes')").bind(id).execute(&a.db).await.unwrap();
    let stats = api::uptime_value(&a, id, 1).await.unwrap();
    assert!((stats["uptime"].as_f64().unwrap() - 50.0).abs() < 0.01);
    assert!((stats["covered_seconds"].as_f64().unwrap() - 80.0).abs() < 0.1);
}
#[sqlx::test(migrations = "../../migrations")]
async fn discord_delivery_retries_and_disables_mentions(pool: sqlx::PgPool) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let a = app(pool);
    let attempts = Arc::new(AtomicUsize::new(0));
    let captured = Arc::new(tokio::sync::Mutex::new(json!(null)));
    let count = attempts.clone();
    let body = captured.clone();
    let mock = Router::new().route(
        "/hook",
        post(move |Json(payload): Json<Value>| {
            let count = count.clone();
            let body = body.clone();
            async move {
                *body.lock().await = payload;
                if count.fetch_add(1, Ordering::SeqCst) == 0 {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::NO_CONTENT
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        axum::serve(listener, mock).await.unwrap();
    });
    let channel = Uuid::new_v4();
    sqlx::query("INSERT INTO notification_channels(id,name,provider,secret) VALUES($1,'Discord mock','discord',$2)").bind(channel).bind(encrypt(&a,&format!("http://127.0.0.1:{port}/hook")).unwrap()).execute(&a.db).await.unwrap();
    sqlx::query("INSERT INTO notification_deliveries(id,channel_id,event,payload) VALUES($1,$2,'DOWN',$3)").bind(Uuid::new_v4()).bind(channel).bind(json!({"state":"DOWN","name":"API","reason":"Timed out","url":"http://test.local/monitors/test"})).execute(&a.db).await.unwrap();
    engine::deliver(&a).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT state FROM notification_deliveries")
        .fetch_one(&a.db)
        .await
        .unwrap();
    assert_eq!(state, "pending");
    sqlx::query("UPDATE notification_deliveries SET due_at=now()")
        .execute(&a.db)
        .await
        .unwrap();
    engine::deliver(&a).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT state FROM notification_deliveries")
        .fetch_one(&a.db)
        .await
        .unwrap();
    assert_eq!(state, "sent");
    let payload = captured.lock().await;
    assert_eq!(payload["allowed_mentions"]["parse"], json!([]));
    assert_eq!(payload["embeds"][0]["title"], "DOWN · API");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    task.abort();
}
#[sqlx::test(migrations = "../../migrations")]
async fn resources_overrides_privacy_and_editorial_incidents(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let (_, m, _) = request(&a, "POST", "/monitors", input(), Some(&cookie)).await;
    let monitor: Uuid = m["id"].as_str().unwrap().parse().unwrap();
    let failure = aegis_checks::Observation::from_error("Private upstream authentication failed");
    engine::record(&a, monitor, failure.clone()).await.unwrap();
    engine::record(&a, monitor, failure).await.unwrap();
    let(_,r,_)=request(&a,"POST","/resources",json!({"name":"Public API","description":"Customer-facing API","monitor_ids":[monitor],"public":true}),Some(&cookie)).await;
    let resource = r["id"].as_str().unwrap();
    let(status,p,_)=request(&a,"POST","/status-pages",json!({"slug":"services","title":"Service health","resource_ids":[resource],"tabs":[{"id":"services","label":"Services","kind":"overview"},{"id":"updates","label":"Updates","kind":"incidents"},{"id":"help","label":"Help","kind":"text","body":"Contact support."}]}),Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let page = p["id"].as_str().unwrap();
    let (_, public, _) = request(&a, "GET", "/public/status/services", json!(null), None).await;
    assert_eq!(public["resources"][0]["state"], "major_outage");
    assert!(!public.to_string().contains("authentication failed"));
    assert!(!public.to_string().contains("127.0.0.1"));
    assert_eq!(public["tabs"][2]["body"], "Contact support.");
    let result=request(&a,"PUT",&format!("/resources/{resource}"),json!({"name":"Public API","monitor_ids":[monitor],"public":true,"override_state":"degraded","override_reason":"Investigating delays","override_until":(chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339()}),Some(&cookie)).await;
    assert_eq!(result.0, StatusCode::OK);
    let (_, public, _) = request(&a, "GET", "/public/status/services", json!(null), None).await;
    assert_eq!(public["resources"][0]["state"], "degraded");
    assert_eq!(public["resources"][0]["status_source"], "manual");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM monitors WHERE id=$1")
            .bind(monitor)
            .fetch_one(&a.db)
            .await
            .unwrap(),
        "DOWN"
    );
    let(_,incident,_)=request(&a,"POST","/incidents",json!({"title":"Intermittent delays","public_message":"Investigating elevated latency.","published":true,"status_page_ids":[page],"resource_ids":[resource],"note":"Private incident coordination password: not-public"}),Some(&cookie)).await;
    let id = incident["id"].as_str().unwrap();
    assert_eq!(
        request(
            &a,
            "POST",
            &format!("/incidents/{id}/updates"),
            json!({"phase":"identified","message":"The cause has been identified."}),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &a,
            "POST",
            &format!("/incidents/{id}/updates"),
            json!({"phase":"resolved","message":"Normal service has returned."}),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, public, _) = request(&a, "GET", "/public/status/services", json!(null), None).await;
    assert_eq!(public["incidents"][0]["phase"], "resolved");
    assert_eq!(
        public["incidents"][0]["updates"].as_array().unwrap().len(),
        3
    );
    assert!(!public.to_string().contains("not-public"));
    assert!(!public.to_string().contains("Private upstream"));
    sqlx::query("UPDATE status_resources SET override_until=now()-interval '1 minute' WHERE id=$1")
        .bind(resource.parse::<Uuid>().unwrap())
        .execute(&a.db)
        .await
        .unwrap();
    let (_, public, _) = request(&a, "GET", "/public/status/services", json!(null), None).await;
    assert_eq!(public["resources"][0]["state"], "major_outage");
    assert_eq!(public["resources"][0]["status_source"], "automatic");
    sqlx::query("UPDATE monitors SET public=false WHERE id=$1")
        .bind(monitor)
        .execute(&a.db)
        .await
        .unwrap();
    let (_, public, _) = request(&a, "GET", "/public/status/services", json!(null), None).await;
    assert_eq!(public["resources"][0]["state"], "unknown");
    assert!(public["resources"][0]["uptime"][0]["uptime"].is_null());
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_audit_events")
        .fetch_one(&a.db)
        .await
        .unwrap();
    assert!(events >= 6);
}
#[sqlx::test(migrations = "../../migrations")]
async fn private_resources_invalid_tabs_and_scope_are_rejected(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let mut m = input();
    m["public"] = json!(false);
    let (_, monitor, _) = request(&a, "POST", "/monitors", m, Some(&cookie)).await;
    let id = monitor["id"].as_str().unwrap();
    assert_eq!(
        request(
            &a,
            "POST",
            "/resources",
            json!({"name":"Private host","monitor_ids":[id],"public":true}),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let(_,r,_)=request(&a,"POST","/resources",json!({"name":"Private service","public":false,"override_state":"operational","override_reason":"Manual administrative state"}),Some(&cookie)).await;
    request(
        &a,
        "POST",
        "/status-pages",
        json!({"slug":"safe","title":"Safe","resource_ids":[r["id"]]}),
        Some(&cookie),
    )
    .await;
    let (_, public, _) = request(&a, "GET", "/public/status/safe", json!(null), None).await;
    assert_eq!(public["resources"], json!([]));
    assert!(!public.to_string().contains("Private service"));
    for tabs in [
        json!([{ "id":"x","label":"HTML","kind":"html","body":"<script>bad</script>"}]),
        json!([{ "id":"x","label":"One","kind":"text"},{"id":"x","label":"Two","kind":"text"}]),
        json!([{ "id":"x","label":"Bad","kind":"overview","secret":"not-allowed"}]),
    ] {
        assert_eq!(
            request(
                &a,
                "POST",
                "/status-pages",
                json!({"slug":"invalid","title":"Invalid","tabs":tabs}),
                Some(&cookie)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let (_, page, _) = request(
        &a,
        "POST",
        "/status-pages",
        json!({"slug":"other","title":"Other"}),
        Some(&cookie),
    )
    .await;
    request(&a,"POST","/incidents",json!({"title":"Private communication","public_message":"Only on other page","published":true,"status_page_ids":[page["id"]]}),Some(&cookie)).await;
    let (_, public, _) = request(&a, "GET", "/public/status/safe", json!(null), None).await;
    assert_eq!(public["incidents"], json!([]));
}
#[sqlx::test(migrations = "../../migrations")]
async fn embed_editing_preserves_secret_and_freezes_routed_jobs(pool: sqlx::PgPool) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let (_, created, _) = request(&a, "POST", "/monitors", input(), Some(&cookie)).await;
    let monitor: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    let (_, other, _) = request(&a, "POST", "/monitors", input(), Some(&cookie)).await;
    let other: Uuid = other["id"].as_str().unwrap().parse().unwrap();
    let template = json!({"title":"Custom {{status}}: {{monitor}}","description":"{{reason}}","down_color":"#123456","fields":[{"name":"Incident","value":"{{incident_id}}","inline":true}]});
    let(status,channel,_)=request(&a,"POST","/channels",json!({"name":"Operations","provider":"webhook","secret":"http://127.0.0.1:19192","monitor_ids":[monitor],"events":["DOWN"],"template":template}),Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let id: Uuid = channel["id"].as_str().unwrap().parse().unwrap();
    let stored: String = sqlx::query_scalar("SELECT secret FROM notification_channels WHERE id=$1")
        .bind(id)
        .fetch_one(&a.db)
        .await
        .unwrap();
    assert_ne!(stored, "http://127.0.0.1:19192");
    assert_eq!(decrypt(&a, &stored).unwrap(), "http://127.0.0.1:19192");
    let f = aegis_checks::Observation::from_error("Connection timed out");
    for m in [monitor, other] {
        for _ in 0..2 {
            engine::record(&a, m, f.clone()).await.unwrap();
        }
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_deliveries")
            .fetch_one(&a.db)
            .await
            .unwrap(),
        1
    );
    let(status,_,_)=request(&a,"PUT",&format!("/channels/{id}"),json!({"name":"Edited operations","provider":"webhook","secret":null,"events":["RECOVERED"],"template":{"title":"New title"}}),Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let current: String =
        sqlx::query_scalar("SELECT secret FROM notification_channels WHERE id=$1")
            .bind(id)
            .fetch_one(&a.db)
            .await
            .unwrap();
    assert_eq!(stored, current);
    let frozen: Value = sqlx::query_scalar("SELECT payload FROM notification_deliveries")
        .fetch_one(&a.db)
        .await
        .unwrap();
    assert_eq!(
        frozen["template"]["title"],
        "Custom {{status}}: {{monitor}}"
    );
    let (_, channels, _) = request(&a, "GET", "/channels", json!(null), Some(&cookie)).await;
    assert!(!channels.to_string().contains("19192"));
    assert!(!channels.to_string().contains(&stored));
    assert_eq!(
        request(
            &a,
            "POST",
            "/channels/preview",
            json!({"template":{"title":"{{secret}}"}}),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, payload, _) = request(
        &a,
        "POST",
        "/channels/preview",
        json!({"template":template,"event":"DOWN"}),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["embeds"][0]["color"], 0x123456);
    assert_eq!(payload["allowed_mentions"]["parse"], json!([]));
}
#[sqlx::test(migrations = "../../migrations")]
async fn monitor_secrets_are_encrypted_redacted_and_used_only_at_authorized_target(
    pool: sqlx::PgPool,
) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let mut m = input();
    m["credentials"] = json!({"bearer":"very-private-api-token","headers":[{"name":"X-API-Key","value":"secret-header-value"}]});
    let (status, value, _) = request(&a, "POST", "/monitors", m.clone(), Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let id = value["id"].as_str().unwrap();
    let stored: String = sqlx::query_scalar("SELECT secret_config FROM monitors WHERE id=$1")
        .bind(id.parse::<Uuid>().unwrap())
        .fetch_one(&a.db)
        .await
        .unwrap();
    assert!(!stored.contains("very-private"));
    assert!(
        decrypt(&a, &stored)
            .unwrap()
            .contains("secret-header-value")
    );
    let (_, monitor, _) = request(
        &a,
        "GET",
        &format!("/monitors/{id}"),
        json!(null),
        Some(&cookie),
    )
    .await;
    assert_eq!(monitor["has_credentials"], true);
    assert!(monitor.get("secret_config").is_none());
    assert!(!monitor.to_string().contains("secret-header-value"));
    let (_, list, _) = request(&a, "GET", "/monitors", json!(null), Some(&cookie)).await;
    assert!(!list.to_string().contains(&stored));
    let mut changed = input();
    changed["target"] = json!("http://127.0.0.1:19193/other");
    changed["monitor_id"] = json!(id);
    assert_eq!(
        request(&a, "POST", "/monitors/test", changed, Some(&cookie))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let mut update = input();
    assert_eq!(
        request(
            &a,
            "PUT",
            &format!("/monitors/{id}"),
            update.clone(),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    update["credentials"] = json!({"clear":true});
    assert_eq!(
        request(&a, "PUT", &format!("/monitors/{id}"), update, Some(&cookie))
            .await
            .0,
        StatusCode::OK
    );
    let (_, monitor, _) = request(
        &a,
        "GET",
        &format!("/monitors/{id}"),
        json!(null),
        Some(&cookie),
    )
    .await;
    assert_eq!(monitor["has_credentials"], false);
    let mut bad = input();
    bad["credentials"] = json!({"headers":[{"name":"Host","value":"other.example"}]});
    assert_eq!(
        request(&a, "POST", "/monitors", bad, Some(&cookie)).await.0,
        StatusCode::BAD_REQUEST
    );
}
#[sqlx::test(migrations = "../../migrations")]
async fn published_maintenance_uses_explicit_safe_copy_and_session_rotation_revokes_old_cookie(
    pool: sqlx::PgPool,
) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let (_, p, _) = request(
        &a,
        "POST",
        "/status-pages",
        json!({"slug":"maintenance","title":"Maintenance status"}),
        Some(&cookie),
    )
    .await;
    let(status,w,_)=request(&a,"POST","/maintenance",json!({"reason":"Private migration connection string secret","title":"Scheduled maintenance","public_message":"We are upgrading the platform.","status_page_ids":[p["id"]],"starts_at":(chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339(),"ends_at":(chrono::Utc::now()+chrono::Duration::hours(2)).to_rfc3339(),"monitor_ids":[]}),Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(w.get("id").is_some());
    let (_, public, _) = request(&a, "GET", "/public/status/maintenance", json!(null), None).await;
    assert_eq!(public["maintenance"][0]["title"], "Scheduled maintenance");
    assert!(!public.to_string().contains("connection string secret"));
    let (_, sessions, _) = request(&a, "GET", "/auth/sessions", json!(null), Some(&cookie)).await;
    assert_eq!(sessions.as_array().unwrap().len(), 1);
    assert_eq!(sessions[0]["current"], true);
    assert!(sessions[0].get("token_hash").is_none());
    let (status, _, fresh) = request(&a, "POST", "/auth/refresh", json!({}), Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let fresh = fresh.unwrap();
    assert_ne!(fresh, cookie);
    assert_eq!(
        request(&a, "GET", "/me", json!(null), Some(&cookie))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&a, "GET", "/me", json!(null), Some(&fresh)).await.0,
        StatusCode::OK
    );
}
#[sqlx::test(migrations = "../../migrations")]
async fn viewer_cannot_publish_or_edit_embeds_and_manual_resources_do_not_invent_uptime(
    pool: sqlx::PgPool,
) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    request(
        &a,
        "POST",
        "/users",
        json!({"email":"viewer@example.test","password":"viewer-long-password","role":"viewer"}),
        Some(&cookie),
    )
    .await;
    let (_, _, viewer) = request(
        &a,
        "POST",
        "/auth/login",
        json!({"email":"viewer@example.test","password":"viewer-long-password"}),
        None,
    )
    .await;
    for (path, body) in [
        ("/resources", json!({"name":"Nope"})),
        ("/incidents", json!({"title":"Nope"})),
        ("/channels/preview", json!({"template":{}})),
        ("/status-pages", json!({"title":"Nope","slug":"nope"})),
    ] {
        assert_eq!(
            request(&a, "POST", path, body, viewer.as_deref()).await.0,
            StatusCode::FORBIDDEN
        );
    }
    let(_,r,_)=request(&a,"POST","/resources",json!({"name":"Manual service","public":true,"override_state":"operational","override_reason":"Operator confirmation"}),Some(&cookie)).await;
    request(
        &a,
        "POST",
        "/status-pages",
        json!({"slug":"manual","title":"Manual","resource_ids":[r["id"]]}),
        Some(&cookie),
    )
    .await;
    let (_, public, _) = request(&a, "GET", "/public/status/manual", json!(null), None).await;
    assert_eq!(public["resources"][0]["state"], "operational");
    for metric in public["resources"][0]["uptime"].as_array().unwrap() {
        assert!(metric["uptime"].is_null());
        assert_eq!(metric["covered_seconds"], 0.0);
    }
}
#[sqlx::test(migrations = false)]
async fn additive_upgrade_preserves_existing_v1_data(pool: sqlx::PgPool) {
    sqlx::raw_sql(include_str!("../../../migrations/0001_initial.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO monitors(id,name,kind,target,interval_seconds,timeout_seconds,failure_threshold,recovery_threshold,state) VALUES($1,'Existing production monitor','tcp','example.com:443',30,10,2,2,'DOWN')").bind(id).execute(&pool).await.unwrap();
    let incident = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO incidents(id,monitor_id,cause) VALUES($1,$2,'Existing private cause')",
    )
    .bind(incident)
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO checks(monitor_id,ok,state,latency_ms) VALUES($1,false,'DOWN',10)")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../../migrations/0002_status_platform.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let row = sqlx::query("SELECT title,published,phase,cause FROM incidents WHERE id=$1")
        .bind(incident)
        .fetch_one(&pool)
        .await
        .unwrap();
    use sqlx::Row;
    assert_eq!(row.get::<String, _>("title"), "Existing production monitor");
    assert!(!row.get::<bool, _>("published"));
    assert_eq!(row.get::<String, _>("phase"), "investigating");
    assert_eq!(row.get::<String, _>("cause"), "Existing private cause");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM checks WHERE monitor_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}
#[sqlx::test(migrations = "../../migrations")]
async fn batched_dashboard_history_and_global_counts_include_private_monitors_without_secrets(
    pool: sqlx::PgPool,
) {
    let a = app(pool);
    let cookie = owner_session(&a).await;
    let mut m = input();
    m["public"] = json!(false);
    m["credentials"] = json!({"bearer":"never-in-summary"});
    let (_, created, _) = request(&a, "POST", "/monitors", m, Some(&cookie)).await;
    let id: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    engine::record(
        &a,
        id,
        aegis_checks::Observation {
            ok: true,
            latency_ms: 1.0,
            error: None,
            observed: json!({}),
        },
    )
    .await
    .unwrap();
    let (status, summary, _) = request(
        &a,
        "GET",
        "/monitors/summaries?days=1",
        json!(null),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(summary.as_array().unwrap().len(), 1);
    assert_eq!(summary[0]["checks"].as_array().unwrap().len(), 1);
    assert!(!summary.to_string().contains("never-in-summary"));
    let (status, counts, _) =
        request(&a, "GET", "/monitors/counts", json!(null), Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(counts["total"], 1);
    assert_eq!(
        request(
            &a,
            "GET",
            "/monitors?limit=1&offset=1",
            json!(null),
            Some(&cookie)
        )
        .await
        .1,
        json!([])
    );
}

#[test]
fn resource_thresholds_use_real_metrics_and_never_default_missing_data_to_healthy() {
    let mut monitor: aegis_core::MonitorInput = serde_json::from_value(json!({
        "name":"Host resources", "kind":"cpu", "target":"host", "config":{"cpu_threshold":80}
    }))
    .expect("valid test monitor");
    let mut inventory = json!({"host":{"cpu_percent":20.0,"memory_percent":40.0,"load":0.5,"disks":[{"mount":"/","total":1000,"available":400},{"mount":"/data","total":1000,"available":10}]}});
    assert!(api::agent_observation(&monitor, &inventory, true).ok);
    inventory["host"]["cpu_percent"] = json!(80);
    assert!(!api::agent_observation(&monitor, &inventory, true).ok);
    monitor.kind = "memory".into();
    monitor.config = json!({"memory_threshold":30});
    assert!(!api::agent_observation(&monitor, &inventory, true).ok);
    monitor.kind = "disk".into();
    monitor.config = json!({"disk_threshold":90,"disk_mount":"/"});
    assert!(api::agent_observation(&monitor, &inventory, true).ok);
    monitor.config["disk_mount"] = json!("/data");
    assert!(!api::agent_observation(&monitor, &inventory, true).ok);
    monitor.config["disk_mount"] = json!("/missing");
    assert!(!api::agent_observation(&monitor, &inventory, true).ok);
    monitor.kind = "host".into();
    monitor.config = json!({"load_threshold":0.25});
    assert!(!api::agent_observation(&monitor, &inventory, true).ok);
    monitor.kind = "cpu".into();
    monitor.config = json!({});
    assert!(!api::agent_observation(&monitor, &json!({"host":{}}), true).ok);
    assert!(!api::agent_observation(&monitor, &inventory, false).ok);
}

#[sqlx::test(migrations = "../../migrations")]
async fn deletion_cleans_memberships_without_widening_routes_or_publication(db: sqlx::PgPool) {
    let monitor = Uuid::new_v4();
    let resource = Uuid::new_v4();
    let page = Uuid::new_v4();
    let incident = Uuid::new_v4();
    let channel = Uuid::new_v4();
    let broadcast = Uuid::new_v4();
    sqlx::query("INSERT INTO monitors(id,name,kind,target,interval_seconds,timeout_seconds,failure_threshold,recovery_threshold) VALUES($1,'test','push','',20,5,1,1)")
        .bind(monitor)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO notification_channels(id,name,provider,secret,monitor_ids) VALUES($1,'scoped','webhook','test',$2),($3,'broadcast','webhook','test','{}')").bind(channel).bind(vec![monitor]).bind(broadcast).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO status_resources(id,name,monitor_ids) VALUES($1,'resource',$2)")
        .bind(resource)
        .bind(vec![monitor])
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO status_pages(id,slug,title,monitor_ids,resource_ids) VALUES($1,'scope','scope',$2,$3)").bind(page).bind(vec![monitor]).bind(vec![resource]).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO incidents(id,title,resource_ids,status_page_ids,published) VALUES($1,'manual',$2,$3,true)").bind(incident).bind(vec![resource]).bind(vec![page]).execute(&db).await.unwrap();
    sqlx::query("DELETE FROM monitors WHERE id=$1")
        .bind(monitor)
        .execute(&db)
        .await
        .unwrap();
    let scoped: bool = sqlx::query_scalar("SELECT enabled FROM notification_channels WHERE id=$1")
        .bind(channel)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!scoped);
    let all: bool = sqlx::query_scalar("SELECT enabled FROM notification_channels WHERE id=$1")
        .bind(broadcast)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(all);
    let memberships:i64=sqlx::query_scalar("SELECT (SELECT cardinality(monitor_ids) FROM status_pages WHERE id=$1)::bigint+(SELECT cardinality(monitor_ids) FROM status_resources WHERE id=$2)::bigint").bind(page).bind(resource).fetch_one(&db).await.unwrap();
    assert_eq!(memberships, 0);
    sqlx::query("DELETE FROM status_resources WHERE id=$1")
        .bind(resource)
        .execute(&db)
        .await
        .unwrap();
    let published: bool = sqlx::query_scalar("SELECT published FROM incidents WHERE id=$1")
        .bind(incident)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!published);
    sqlx::query("DELETE FROM status_pages WHERE id=$1")
        .bind(page)
        .execute(&db)
        .await
        .unwrap();
    let scopes: i32 =
        sqlx::query_scalar("SELECT cardinality(status_page_ids) FROM incidents WHERE id=$1")
            .bind(incident)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(scopes, 0);
}
