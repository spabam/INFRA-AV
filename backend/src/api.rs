use actix_cors::Cors;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, web};
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::json;
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::auth;
use crate::browser;
use crate::clamav::probe_command;
use crate::config::AppConfig;
use crate::models::{
    ApiEnvelope, ApiResponse, HealthResponse, JobStatus, LoginRequest, LoginResponse,
    ScanActionRequest, ScanBrowseRequest, ScanControlRequest, ScanControlResponse, ScanListRequest,
    ScanListResponse, ScanReportResponse, ScanStatusRequest, ScanStatusResponse,
    SignatureUpdateResponse,
};
use crate::{queue, report, worker};

const FRESHCLAM_LOCK_FILE: &str = "/var/lock/infra-av-freshclam.lock";

#[derive(Clone)]
struct AppState {
    config: AppConfig,
}

pub async fn run_server(config: AppConfig) -> Result<()> {
    let bind_address = format!("{}:{}", config.host, config.port);
    let state = AppState {
        config: config.clone(),
    };

    HttpServer::new(move || {
        let cors = build_cors(&state.config);
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(cors)
            // Legacy action API ({action,payload}); accepted with and without a
            // trailing slash so the UUUUI gateway (/api/av/ -> /api/) and the
            // standalone/embedded paths (-> /api) both reach it.
            .route("/api", web::post().to(api_handler))
            .route("/api/", web::post().to(api_handler))
            // UUUUI API contract v1 surface (GET; liveness/schema need no auth —
            // the gateway enforces the module grant). Envelope {ok,data}.
            .route("/api/health", web::get().to(health_route))
            .route("/api/capabilities", web::get().to(capabilities_route))
            .route("/api/openapi.json", web::get().to(openapi_route))
    })
    .bind(&bind_address)
    .with_context(|| format!("failed to bind {bind_address}"))?
    .run()
    .await
    .context("actix server failed")
}

async fn api_handler(
    request: HttpRequest,
    body: web::Json<ApiEnvelope>,
    state: web::Data<AppState>,
) -> HttpResponse {
    match dispatch(request, body.into_inner(), state.get_ref()).await {
        Ok(response) => response,
        Err(error) => map_error(error),
    }
}

// --- UUUUI API contract v1 surface -----------------------------------------
const CONTRACT_VERSION: &str = "1";
const OPENAPI_JSON: &str = include_str!("openapi.json");

/// GET /api/health — standard envelope (contract §5). No auth (liveness).
async fn health_route() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::success(json!({
        "status": "ok",
        "version": crate::version::VERSION,
        "contract": CONTRACT_VERSION,
    })))
}

/// GET /api/capabilities — standard envelope (contract §5).
async fn capabilities_route() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::success(json!({
        "available": true,
        "version": crate::version::VERSION,
        "contract": CONTRACT_VERSION,
        // The scan API is still the {action,payload} dispatch (normalized by the
        // shared client); full REST conversion is the remaining Tier-A step.
        "api": "action-dispatch",
        "features": {
            "scan": true,
            "signatures": true,
            "history": true,
        },
    })))
}

/// GET /api/openapi.json — the raw OpenAPI 3.1 document (not enveloped).
async fn openapi_route() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(OPENAPI_JSON)
}

async fn dispatch(
    request: HttpRequest,
    envelope: ApiEnvelope,
    state: &AppState,
) -> Result<HttpResponse> {
    match envelope.action.as_str() {
        "auth.login" => {
            if auth::directory_enabled() {
                return Ok(forbidden("Sign in through UUUUI"));
            }
            let payload: LoginRequest =
                serde_json::from_value(envelope.payload).context("invalid login payload")?;
            let role = auth::authenticate(&state.config, &payload.username, &payload.password)?;
            let token = auth::issue_token(&state.config, &payload.username, &role)?;
            Ok(ok(LoginResponse {
                token,
                username: payload.username,
                role,
            }))
        }
        "auth.sso" => {
            if auth::directory_enabled() {
                let claims = claims_from_request(&request, &state.config)?;
                let token = auth::issue_token(&state.config, &claims.sub, &claims.role)?;
                return Ok(ok(LoginResponse {
                    token,
                    username: claims.sub,
                    role: claims.role,
                }));
            }
            let secret = state
                .config
                .broker_secret
                .as_deref()
                .ok_or_else(|| anyhow!("sso not configured"))?;
            #[derive(serde::Deserialize)]
            struct SsoRequest {
                token: String,
            }
            let payload: SsoRequest =
                serde_json::from_value(envelope.payload).context("invalid sso payload")?;
            let (username, roles) = auth::verify_broker_token(secret, &payload.token)?;
            // UUUUI admins/superadmins map to the AV admin role; everyone else to
            // the configured default role.
            let role = if roles.iter().any(|r| r == "admin" || r == "superadmin") {
                "admin".to_string()
            } else {
                state.config.default_role.clone()
            };
            let token = auth::issue_token(&state.config, &username, &role)?;
            Ok(ok(LoginResponse {
                token,
                username,
                role,
            }))
        }
        "health.check" => {
            let counts = queue::queue_counts(&state.config)?;
            let (clamscan_ok, clamscan_output) =
                probe_command(&state.config.clamscan_bin, "--version");
            let (freshclam_ok, freshclam_output) =
                probe_command(&state.config.freshclam_bin, "--version");
            Ok(ok(HealthResponse {
                product_version: crate::version::VERSION.to_string(),
                backend_version: crate::version::BACKEND_VERSION.to_string(),
                clamscan: crate::models::CommandProbe {
                    ok: clamscan_ok,
                    output: clamscan_output,
                },
                freshclam: crate::models::CommandProbe {
                    ok: freshclam_ok,
                    output: freshclam_output,
                },
                queue: counts,
            }))
        }
        "scan.submit" => {
            let claims = claims_from_request(&request, &state.config)?;
            let payload: ScanActionRequest =
                serde_json::from_value(envelope.payload).context("invalid scan payload")?;
            let response = queue::submit_job(&state.config, &claims.sub, payload)?;
            Ok(ok(response))
        }
        "scan.browse" => {
            let _claims = claims_from_request(&request, &state.config)?;
            let payload: ScanBrowseRequest =
                serde_json::from_value(envelope.payload).context("invalid browse payload")?;
            let response = browser::browse_targets(&state.config, payload)?;
            Ok(ok(response))
        }
        "scan.status" => {
            let claims = claims_from_request(&request, &state.config)?;
            let payload: ScanStatusRequest =
                serde_json::from_value(envelope.payload).context("invalid status payload")?;
            let job = queue::load_job(&state.config, &payload.request_id)?
                .filter(|job| claims.role == "admin" || job.submitted_by == claims.sub)
                .ok_or_else(|| anyhow!("job not found"))?;
            let result = job.result.clone();
            let report_available = result.is_some();
            let report_filename =
                report_available.then(|| report::report_filename(&job.request_id));
            Ok(ok(ScanStatusResponse {
                request_id: job.request_id,
                status: job.status,
                target: job.target,
                submitted_by: job.submitted_by,
                submitted_at: job.submitted_at,
                started_at: result
                    .as_ref()
                    .and_then(|value| value.started_at)
                    .or(job.execution.as_ref().map(|value| value.started_at)),
                finished_at: result.as_ref().and_then(|value| value.finished_at),
                command_preview: job.command_preview,
                output: result.clone().map(|value| value.output).unwrap_or_default(),
                summary: result.clone().and_then(|value| value.summary),
                error_message: result.and_then(|value| value.error_message),
                report_available,
                report_filename,
            }))
        }
        "scan.report" => {
            let claims = claims_from_request(&request, &state.config)?;
            let payload: ScanStatusRequest =
                serde_json::from_value(envelope.payload).context("invalid report payload")?;
            let job = queue::load_job(&state.config, &payload.request_id)?
                .filter(|job| claims.role == "admin" || job.submitted_by == claims.sub)
                .ok_or_else(|| anyhow!("job not found"))?;
            let (filename, html) = report::read_or_render_report(&state.config, &job)?;
            Ok(ok(ScanReportResponse {
                request_id: job.request_id,
                filename,
                html,
            }))
        }
        "scan.list" => {
            let claims = claims_from_request(&request, &state.config)?;
            let payload: ScanListRequest =
                serde_json::from_value(envelope.payload).unwrap_or(ScanListRequest { limit: 20 });
            // Non-admin callers only see their own scans; admins see every scan.
            let owner = (claims.role != "admin").then_some(claims.sub.as_str());
            let items = queue::list_jobs(&state.config, payload.limit, owner)?;
            Ok(ok(ScanListResponse { items }))
        }
        "scan.pause" => {
            let claims = claims_from_request(&request, &state.config)?;
            let payload: ScanControlRequest =
                serde_json::from_value(envelope.payload).context("invalid pause payload")?;
            let response =
                update_running_job(&state.config, &claims, &payload.request_id, |job, _path| {
                    ensure_manageable_job(&claims, job)?;
                    if job.status == JobStatus::Paused {
                        return Ok(job.status);
                    }
                    if job.status != JobStatus::Processing {
                        return Err(anyhow!("scan is not currently running"));
                    }
                    let pid = running_child_pid(job)?;
                    send_signal(pid, libc::SIGSTOP)?;
                    job.status = JobStatus::Paused;
                    job.status_updated_at = chrono::Utc::now();
                    if let Some(execution) = job.execution.as_mut() {
                        execution.paused = true;
                    }
                    Ok(job.status)
                })?;
            Ok(ok(response))
        }
        "scan.resume" => {
            let claims = claims_from_request(&request, &state.config)?;
            let payload: ScanControlRequest =
                serde_json::from_value(envelope.payload).context("invalid resume payload")?;
            let response =
                update_running_job(&state.config, &claims, &payload.request_id, |job, _path| {
                    ensure_manageable_job(&claims, job)?;
                    if job.status == JobStatus::Processing {
                        return Ok(job.status);
                    }
                    if job.status != JobStatus::Paused {
                        return Err(anyhow!("scan is not paused"));
                    }
                    let pid = running_child_pid(job)?;
                    send_signal(pid, libc::SIGCONT)?;
                    job.status = JobStatus::Processing;
                    job.status_updated_at = chrono::Utc::now();
                    if let Some(execution) = job.execution.as_mut() {
                        execution.paused = false;
                    }
                    Ok(job.status)
                })?;
            Ok(ok(response))
        }
        "scan.stop" => {
            let claims = claims_from_request(&request, &state.config)?;
            let payload: ScanControlRequest =
                serde_json::from_value(envelope.payload).context("invalid stop payload")?;
            let Some((job, path)) = queue::load_job_with_path(&state.config, &payload.request_id)?
            else {
                return Err(anyhow!("job not found"));
            };

            ensure_manageable_job(&claims, &job)?;
            let response = if path.starts_with(&state.config.pending_dir) {
                let cancelled = queue::cancel_pending_job(&state.config, &payload.request_id)?
                    .ok_or_else(|| {
                        anyhow!("scan started while cancellation was requested; retry Stop")
                    })?;
                ScanControlResponse {
                    request_id: cancelled.request_id,
                    status: cancelled.status,
                }
            } else if path.starts_with(&state.config.processing_dir) {
                let mut job = job;
                let pid = running_child_pid(&job)?;
                if job.status == JobStatus::Paused {
                    send_signal(pid, libc::SIGCONT).ok();
                }
                if let Some(execution) = job.execution.as_mut() {
                    execution.paused = false;
                    execution.cancel_requested = true;
                }
                job.status = JobStatus::Cancelled;
                job.status_updated_at = chrono::Utc::now();
                queue::write_job(&path, &job)?;
                stop_process(pid);
                ScanControlResponse {
                    request_id: job.request_id,
                    status: job.status,
                }
            } else {
                ScanControlResponse {
                    request_id: job.request_id,
                    status: job.status,
                }
            };
            Ok(ok(response))
        }
        "scan.runWorkerOnce" => {
            let claims = claims_from_request(&request, &state.config)?;
            if claims.role != "admin" {
                return Ok(forbidden("admin role required"));
            }
            let processed = worker::run(state.config.clone(), true, std::time::Duration::ZERO);
            match processed {
                Ok(()) => Ok(ok(json!({ "processed": true }))),
                Err(error) => Ok(server_error(error.to_string())),
            }
        }
        "signatures.update" => {
            let claims = claims_from_request(&request, &state.config)?;
            if claims.role != "admin" {
                return Ok(forbidden("admin role required"));
            }
            let (exit_code, lines) = run_signature_update(&state.config)?;

            Ok(ok(SignatureUpdateResponse {
                exit_code,
                output: lines,
            }))
        }
        other => Ok(bad_request(format!("unsupported action {}", other))),
    }
}

fn claims_from_request(request: &HttpRequest, config: &AppConfig) -> Result<auth::Claims> {
    if auth::directory_enabled() {
        let assertion = request
            .headers()
            .get("X-UUUUI-Identity")
            .ok_or_else(|| anyhow!("invalid token: UUUUI sign-in is required"))?
            .to_str()
            .context("invalid token: directory identity header")?;
        return auth::directory_claims(config, assertion);
    }
    let auth_header = request
        .headers()
        .get("Authorization")
        .ok_or_else(|| anyhow!("missing authorization header"))?
        .to_str()
        .context("invalid authorization header")?;
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| anyhow!("authorization header must use Bearer"))?;
    let claims = auth::decode_token(config, token)?;
    // Standalone sessions carry an 8 h token; re-check the current user store so
    // a role change to a locally-managed account is honored immediately.
    auth::reauthorize_native(config, &claims)?;
    Ok(claims)
}

fn build_cors(config: &AppConfig) -> Cors {
    let mut cors = Cors::default()
        .allow_any_header()
        .allowed_methods(vec!["POST"])
        .supports_credentials();
    for origin in &config.cors_origins {
        cors = cors.allowed_origin(origin);
    }
    cors
}

fn update_running_job<F>(
    config: &AppConfig,
    claims: &auth::Claims,
    request_id: &str,
    mut updater: F,
) -> Result<ScanControlResponse>
where
    F: FnMut(&mut crate::models::QueuedJob, &std::path::Path) -> Result<JobStatus>,
{
    let Some((mut job, path)) = queue::load_job_with_path(config, request_id)? else {
        return Err(anyhow!("job not found"));
    };
    ensure_manageable_job(claims, &job)?;
    if !path.starts_with(&config.processing_dir) {
        return Err(anyhow!("scan is not active"));
    }
    let status = updater(&mut job, &path)?;
    queue::write_job(&path, &job)?;
    Ok(ScanControlResponse {
        request_id: job.request_id,
        status,
    })
}

fn ensure_manageable_job(claims: &auth::Claims, job: &crate::models::QueuedJob) -> Result<()> {
    if claims.role == "admin" || claims.sub == job.submitted_by {
        return Ok(());
    }

    Err(anyhow!("you do not have permission to manage this scan"))
}

fn running_child_pid(job: &crate::models::QueuedJob) -> Result<i32> {
    job.execution
        .as_ref()
        .and_then(|value| value.child_pid)
        .map(|value| value as i32)
        .ok_or_else(|| anyhow!("scan process is not available"))
}

fn send_signal(pid: i32, signal: i32) -> Result<()> {
    let rc = unsafe { libc::kill(pid, signal) };
    if rc == 0 {
        Ok(())
    } else {
        Err(anyhow!(
            "failed to signal process {pid}: {}",
            std::io::Error::last_os_error()
        ))
    }
}

fn process_exists(pid: i32) -> bool {
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn stop_process(pid: i32) {
    let _ = send_signal(pid, libc::SIGTERM);
    for _ in 0..20 {
        if !process_exists(pid) {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = send_signal(pid, libc::SIGKILL);
}

fn run_signature_update(config: &AppConfig) -> Result<(i32, Vec<String>)> {
    let output = Command::new("flock")
        .args(["-n", FRESHCLAM_LOCK_FILE, &config.freshclam_bin, "--stdout"])
        .output()
        .with_context(|| format!("failed to execute {}", config.freshclam_bin))?;

    let mut lines = Vec::new();
    lines.extend(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string),
    );
    lines.extend(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .map(str::to_string),
    );

    Ok((output.status.code().unwrap_or(56), lines))
}

fn ok<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::success(data))
}

fn bad_request(message: impl Into<String>) -> HttpResponse {
    HttpResponse::BadRequest()
        .json(ApiResponse::<serde_json::Value>::failure_with_code("invalid", message))
}

fn unauthorized(message: impl Into<String>) -> HttpResponse {
    HttpResponse::Unauthorized()
        .json(ApiResponse::<serde_json::Value>::failure_with_code("unauthenticated", message))
}

fn forbidden(message: impl Into<String>) -> HttpResponse {
    HttpResponse::Forbidden()
        .json(ApiResponse::<serde_json::Value>::failure_with_code("forbidden", message))
}

fn server_error(message: impl Into<String>) -> HttpResponse {
    HttpResponse::InternalServerError()
        .json(ApiResponse::<serde_json::Value>::failure_with_code("internal", message))
}

// Map an internal error to the canonical envelope + HTTP status (contract §3).
fn map_error(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    let normalized = message.to_ascii_lowercase();

    if normalized.contains("invalid token")
        || normalized.contains("missing authorization header")
        || normalized.contains("authorization header must use bearer")
    {
        return unauthorized(message);
    }
    if normalized.contains("not found") {
        return HttpResponse::NotFound()
            .json(ApiResponse::<serde_json::Value>::failure_with_code("not_found", message));
    }
    if normalized.contains("permission") {
        return forbidden(message);
    }

    bad_request(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::App;
    use actix_web::test as actix_test;

    #[actix_web::test]
    async fn health_is_a_standard_envelope() {
        let app = actix_test::init_service(App::new().route("/api/health", web::get().to(health_route))).await;
        let req = actix_test::TestRequest::get().uri("/api/health").to_request();
        let body: serde_json::Value = actix_test::call_and_read_body_json(&app, req).await;
        assert_eq!(body["ok"], serde_json::json!(true));
        assert_eq!(body["data"]["status"], "ok");
        assert_eq!(body["data"]["contract"], CONTRACT_VERSION);
    }

    #[actix_web::test]
    async fn capabilities_declares_the_contract() {
        let app = actix_test::init_service(App::new().route("/api/capabilities", web::get().to(capabilities_route))).await;
        let req = actix_test::TestRequest::get().uri("/api/capabilities").to_request();
        let body: serde_json::Value = actix_test::call_and_read_body_json(&app, req).await;
        assert_eq!(body["ok"], serde_json::json!(true));
        assert_eq!(body["data"]["available"], serde_json::json!(true));
        assert_eq!(body["data"]["contract"], CONTRACT_VERSION);
    }

    #[actix_web::test]
    async fn openapi_is_served_and_parses() {
        let app = actix_test::init_service(App::new().route("/api/openapi.json", web::get().to(openapi_route))).await;
        let req = actix_test::TestRequest::get().uri("/api/openapi.json").to_request();
        let body = actix_test::call_and_read_body(&app, req).await;
        let doc: serde_json::Value = serde_json::from_slice(&body).expect("openapi.json must be valid JSON");
        assert_eq!(doc["openapi"], "3.1.0");
        assert!(doc["paths"]["/health"].is_object());
    }

    #[test]
    fn error_envelope_carries_a_canonical_code() {
        let resp = ApiResponse::<serde_json::Value>::failure_with_code("forbidden", "nope");
        let value = serde_json::to_value(&resp).unwrap();
        assert_eq!(value["ok"], serde_json::json!(false));
        assert_eq!(value["error"]["code"], "forbidden");
        assert_eq!(value["error"]["message"], "nope");
    }
}
