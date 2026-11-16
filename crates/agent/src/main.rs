use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use sysinfo::{Disks, Networks, System};
use tokio::sync::RwLock;
use uuid::Uuid;
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let base = std::env::var("AEGIS_URL").context("AEGIS_URL required")?;
    anyhow::ensure!(
        base.starts_with("https://")
            || std::env::var("ALLOW_INSECURE_HTTP").as_deref() == Ok("true"),
        "Use HTTPS or explicitly set ALLOW_INSECURE_HTTP=true for local development"
    );
    let token = std::env::var("AEGIS_AGENT_TOKEN").context("AEGIS_AGENT_TOKEN required")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let adapters = Arc::new(RwLock::new(
        json!({"docker":[],"pm2":[],"docker_error":"Discovery starting","pm2_errors":[]}),
    ));
    let cache = adapters.clone();
    tokio::spawn(async move {
        loop {
            let (docker, pm2) = tokio::join!(
                tokio::time::timeout(Duration::from_secs(10), docker_inventory()),
                tokio::time::timeout(Duration::from_secs(10), pm2_inventory())
            );
            let docker = docker.unwrap_or_else(|_| {
                (
                    vec![],
                    Some("Docker discovery exceeded its ten-second budget".into()),
                )
            });
            let pm2 = pm2.unwrap_or_else(|_| {
                (
                    vec![],
                    vec!["PM2 discovery exceeded its ten-second budget".into()],
                )
            });
            *cache.write().await =
                json!({"docker":docker.0,"docker_error":docker.1,"pm2":pm2.0,"pm2_errors":pm2.1});
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
    let mut system = System::new_all();
    let mut networks = Networks::new_with_refreshed_list();
    loop {
        system.refresh_all();
        networks.refresh(true);
        let disks = Disks::new_with_refreshed_list();
        let disk_values:Vec<_>=disks.iter().map(|d|json!({"mount":d.mount_point().to_string_lossy(),"total":d.total_space(),"available":d.available_space()})).collect();
        let net:Vec<_>=networks.iter().map(|(name,n)|json!({"name":name,"received":n.total_received(),"transmitted":n.total_transmitted()})).collect();
        let cached = adapters.read().await.clone();
        let inventory = json!({"host":{"hostname":System::host_name(),"cpu_percent":system.global_cpu_usage(),"load":System::load_average().one,"memory_total":system.total_memory(),"memory_used":system.used_memory(),"memory_percent":100.0*system.used_memory() as f64/system.total_memory().max(1) as f64,"uptime_seconds":System::uptime(),"disks":disk_values,"network":net},"docker":cached["docker"],"docker_error":cached["docker_error"],"pm2":cached["pm2"],"pm2_errors":cached["pm2_errors"]});
        let report = json!({"submission_id":Uuid::new_v4(),"version":env!("CARGO_PKG_VERSION"),"inventory":inventory});
        if report.to_string().len() > 200000 {
            tracing::error!("Inventory exceeds 200 KB limit; reduce adapter scope");
        } else {
            for attempt in 0..3 {
                match client
                    .post(format!(
                        "{}/api/v1/agent/heartbeat",
                        base.trim_end_matches('/')
                    ))
                    .bearer_auth(&token)
                    .json(&report)
                    .send()
                    .await
                {
                    Ok(r) if r.status().is_success() => break,
                    Ok(r) => tracing::warn!(status=%r.status(),"Heartbeat rejected"),
                    Err(_) => tracing::warn!("Central API unreachable"),
                };
                tokio::time::sleep(Duration::from_secs(2_u64.pow(attempt))).await;
            }
        }
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}
async fn docker_inventory() -> (Vec<Value>, Option<String>) {
    if std::env::var("AEGIS_DOCKER").as_deref() != Ok("true") {
        return (vec![], Some("Docker adapter disabled".into()));
    }
    let docker = match bollard::Docker::connect_with_local_defaults() {
        Ok(d) => d,
        Err(_) => return (vec![], Some("Docker socket unavailable".into())),
    };
    let options = bollard::query_parameters::ListContainersOptionsBuilder::default()
        .all(true)
        .build();
    match tokio::time::timeout(
        Duration::from_secs(5),
        docker.list_containers(Some(options)),
    )
    .await
    {
        Ok(Ok(containers)) => {
            let mut result = vec![];
            for c in containers.into_iter().take(200) {
                let id = c.id.unwrap_or_default();
                let detail = tokio::time::timeout(
                    Duration::from_secs(2),
                    docker.inspect_container(
                        &id,
                        None::<bollard::query_parameters::InspectContainerOptions>,
                    ),
                )
                .await;
                let inspected = matches!(&detail, Ok(Ok(_)));
                let (health, restarts) = match detail {
                    Ok(Ok(d)) => {
                        let health = d
                            .state
                            .and_then(|s| s.health)
                            .and_then(|h| h.status)
                            .map(|s| s.to_string());
                        (health, d.restart_count)
                    }
                    _ => (None, None),
                };
                let running = c.state.as_ref().is_some_and(|s| s.to_string() == "running");
                result.push(json!({"id":id,"name":c.names.unwrap_or_default().join(","),"state":c.state,"status":c.status,"health":health,"healthy":inspected&&running&&health.as_deref()!=Some("unhealthy")&&health.as_deref()!=Some("starting"),"restarts":restarts,"labels":c.labels}));
            }
            (result, None)
        }
        _ => (vec![], Some("Docker adapter failed or timed out".into())),
    }
}
async fn pm2_inventory() -> (Vec<Value>, Vec<String>) {
    // Paths are operator-configured, never supplied through a monitor/API.
    let homes = std::env::var("AEGIS_PM2_HOMES").unwrap_or_default();
    let binary = std::env::var("AEGIS_PM2_BINARY").unwrap_or_else(|_| "pm2".into());
    let mut result = vec![];
    let mut errors = vec![];
    for home in homes.split(':').filter(|s| !s.is_empty()).take(10) {
        let mut command = tokio::process::Command::new(&binary);
        command
            .arg("jlist")
            .env("PM2_HOME", home)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let output = read_bounded(command).await;
        match output {
            Ok(bytes) => match serde_json::from_slice::<Vec<Value>>(&bytes) {
                Ok(apps) => {
                    for app in apps.into_iter().take(200) {
                        let env = &app["pm2_env"];
                        let id = format!("{}:{}", home, app["pm_id"]);
                        result.push(json!({"id":id,"name":app["name"],"pm_id":app["pm_id"],"status":env["status"],"healthy":env["status"]=="online","restarts":env["restart_time"],"uptime":env["pm_uptime"],"cpu":app["monit"]["cpu"],"memory":app["monit"]["memory"]}));
                    }
                }
                Err(_) => errors.push("Invalid PM2 JSON output".into()),
            },
            Err(_) => errors.push("PM2 adapter failed, exceeded 1 MiB, or timed out".into()),
        }
    }
    (result, errors)
}
async fn read_bounded(mut command: tokio::process::Command) -> Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut child = command.spawn()?;
        let mut stdout = child.stdout.take().context("No stdout")?.take(1_048_577);
        let mut bytes = vec![];
        stdout.read_to_end(&mut bytes).await?;
        anyhow::ensure!(bytes.len() <= 1_048_576, "Output too large");
        anyhow::ensure!(child.wait().await?.success(), "Command failed");
        Ok(bytes)
    })
    .await?
}
