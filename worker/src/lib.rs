use game_core::{
    GameState, offline_gain,
    save::{CAFE_CREDIT_START, SAVE_SCHEMA_VERSION, SaveError, load_save, to_json, validate},
};
use serde::Deserialize;
use serde_json::{json, value::RawValue};
use worker::*;

const MAX_BODY_BYTES: usize = 8 * 1024;
const MAX_TELEMETRY_BYTES: usize = 1024;
const COOKIE_MAX_AGE_SECS: u32 = 31_536_000;
const COOKIE_NAME: &str = "__Host-pid";
/// Closer PUTs would each earn the 10 s time slack again.
const MIN_PUT_GAP_MS: f64 = 5000.0;

#[derive(Deserialize)]
struct SaveRow {
    state: String,
    schema_version: u32,
    version: u64,
    updated_at: f64,
    cafe_credit: f64,
}

#[derive(Deserialize)]
struct PutBody {
    state: Box<RawValue>,
    version: u64,
    schema_version: u32,
}

#[derive(Deserialize)]
struct VersionRow {
    version: u64,
}

fn reply(status: u16, body: impl ToString) -> Result<Response> {
    let mut res =
        Response::from_body(ResponseBody::Body(body.to_string().into_bytes()))?.with_status(status);
    let headers = res.headers_mut();
    headers.set("Content-Type", "application/json")?;
    headers.set("Cache-Control", "no-store")?;
    Ok(res)
}

fn error(status: u16, message: &str) -> Result<Response> {
    reply(status, json!({ "error": message }))
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

fn cookie_pid(req: &Request) -> Result<Option<String>> {
    let header = req.headers().get("Cookie")?.unwrap_or_default();
    Ok(header
        .split(';')
        .filter_map(|c| {
            c.trim()
                .strip_prefix(COOKIE_NAME)
                .and_then(|c| c.strip_prefix('='))
        })
        .find(|v| is_uuid(v))
        .map(str::to_owned))
}

fn new_uuid() -> Result<String> {
    let crypto = js_sys::Reflect::get(&js_sys::global(), &"crypto".into())?;
    let random_uuid: js_sys::Function = js_sys::Reflect::get(&crypto, &"randomUUID".into())?.into();
    random_uuid
        .call0(&crypto)?
        .as_string()
        .ok_or_else(|| Error::RustError("randomUUID returned a non-string".into()))
}

fn set_cookie(res: &mut Response, pid: &str) -> Result<()> {
    res.headers_mut().append(
        "Set-Cookie",
        &format!(
            "{COOKIE_NAME}={pid}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={COOKIE_MAX_AGE_SECS}"
        ),
    )
}

fn now_ms() -> f64 {
    Date::now().as_millis() as f64
}

async fn load_row(db: &D1Database, pid: &str) -> Result<Option<SaveRow>> {
    db.prepare(
        "SELECT state, schema_version, version, updated_at, cafe_credit FROM saves WHERE player_id = ?1",
    )
    .bind(&[pid.into()])?
    .first::<SaveRow>(None)
    .await
}

fn changes(result: &D1Result) -> Result<usize> {
    Ok(result.meta()?.and_then(|m| m.changes).unwrap_or(0))
}

fn corrupt(e: impl std::fmt::Display) -> Error {
    Error::RustError(format!("stored save is unreadable: {e}"))
}

fn load_row_state(row: &SaveRow) -> Result<GameState> {
    Ok(load_save(row.schema_version, &row.state)
        .map_err(corrupt)?
        .0)
}

async fn get_save(req: Request, env: Env) -> Result<Response> {
    let pid = match cookie_pid(&req)? {
        Some(pid) => pid,
        None => new_uuid()?,
    };
    let mut res = get_save_body(&pid, env).await?;
    set_cookie(&mut res, &pid)?;
    Ok(res)
}

async fn get_save_body(pid: &str, env: Env) -> Result<Response> {
    let db = env.d1("DB")?;
    let Some(row) = load_row(&db, pid).await? else {
        return reply(200, json!({"state": null, "version": 0, "offline": 0}));
    };
    let mut state = load_row_state(&row)?;
    let offline = offline_gain(&state, (now_ms() - row.updated_at) / 1000.0);
    state.bank += offline;
    state.total += offline;
    reply(
        200,
        format!(
            r#"{{"state":{},"version":{},"offline":{}}}"#,
            to_json(&state),
            row.version,
            serde_json::to_string(&offline).map_err(corrupt)?
        ),
    )
}

/// Browsers add Sec-Fetch-Site to every request. A sibling subdomain is same-site but not
/// same-origin. JSON content type forces a CORS preflight for cross-origin scripts, which
/// this Worker never answers.
fn reject_cross_origin(req: &Request) -> Result<Option<Response>> {
    if let Some(site) = req.headers().get("Sec-Fetch-Site")?
        && site != "same-origin"
    {
        return error(403, "cross-origin request").map(Some);
    }
    let content_type = req.headers().get("Content-Type")?.unwrap_or_default();
    let media_type = content_type.split(';').next().unwrap_or_default().trim();
    if !media_type.eq_ignore_ascii_case("application/json") {
        return error(415, "content type must be application/json").map(Some);
    }
    Ok(None)
}

/// Checks Content-Length before reading; None means the body is over `max` bytes.
async fn read_limited(req: &mut Request, max: usize) -> Result<Option<Vec<u8>>> {
    let declared = req.headers().get("Content-Length")?;
    if declared
        .and_then(|n| n.parse::<usize>().ok())
        .is_some_and(|n| n > max)
    {
        return Ok(None);
    }
    let bytes = req.bytes().await?;
    Ok((bytes.len() <= max).then_some(bytes))
}

async fn put_save(mut req: Request, env: Env) -> Result<Response> {
    if let Some(res) = reject_cross_origin(&req)? {
        return Ok(res);
    }
    let Some(pid) = cookie_pid(&req)? else {
        return error(401, "missing session");
    };
    let Some(bytes) = read_limited(&mut req, MAX_BODY_BYTES).await? else {
        return error(413, "body too large");
    };
    let Ok(body) = serde_json::from_slice::<PutBody>(&bytes) else {
        return error(400, "malformed body");
    };
    let (loaded, mut violations) = match load_save(body.schema_version, body.state.get()) {
        Ok(loaded) => loaded,
        Err(SaveError::UnsupportedVersion(_)) => return error(400, "unsupported schema"),
        Err(SaveError::Malformed(_)) => return error(400, "malformed state"),
    };

    let db = env.d1("DB")?;
    let now = now_ms();
    let row = load_row(&db, &pid).await?;
    let (prev, elapsed, credit) = match &row {
        None if body.version != 0 => {
            return reply(409, json!({"state": null, "version": 0}));
        }
        None => (GameState::default(), None, CAFE_CREDIT_START),
        Some(row) if row.version != body.version => {
            return reply(
                409,
                format!(
                    r#"{{"state":{},"version":{}}}"#,
                    to_json(&load_row_state(row)?),
                    row.version
                ),
            );
        }
        Some(row) if now - row.updated_at < MIN_PUT_GAP_MS => {
            let mut res = error(429, "saving too often")?;
            res.headers_mut().set("Retry-After", "5")?;
            return Ok(res);
        }
        Some(row) => (
            load_row_state(row)?,
            Some((now - row.updated_at) / 1000.0),
            row.cafe_credit,
        ),
    };

    let checked = validate(&prev, loaded, elapsed, credit);
    violations.extend(checked.violations);
    let state_json = to_json(&checked.state);
    let new_version = body.version + 1;

    let statement = match &row {
        None => db.prepare(
            "INSERT INTO saves (player_id, state, schema_version, version, updated_at, cafe_credit) \
             VALUES (?1, ?2, ?3, 1, ?4, ?5) ON CONFLICT (player_id) DO NOTHING",
        ).bind(&[
            pid.as_str().into(),
            state_json.as_str().into(),
            SAVE_SCHEMA_VERSION.into(),
            now.into(),
            checked.cafe_credit.into(),
        ])?,
        Some(_) => db.prepare(
            "UPDATE saves SET state = ?1, schema_version = ?2, version = version + 1, \
             updated_at = ?3, cafe_credit = ?4 WHERE player_id = ?5 AND version = ?6",
        ).bind(&[
            state_json.as_str().into(),
            SAVE_SCHEMA_VERSION.into(),
            now.into(),
            checked.cafe_credit.into(),
            pid.as_str().into(),
            (body.version as f64).into(),
        ])?,
    };
    if changes(&statement.run().await?)? == 0 {
        return reply(409, json!({"state": null, "version": body.version}));
    }

    if !violations.is_empty() {
        console_log!(
            "{}",
            json!({
                "event": "save_clamped",
                "player": &pid[..8],
                "violations": format!("{violations:?}"),
            })
        );
    }
    reply(
        200,
        format!(
            r#"{{"version":{new_version},"state":{state_json},"clamped":{}}}"#,
            !violations.is_empty()
        ),
    )
}

async fn reset(req: Request, env: Env) -> Result<Response> {
    if let Some(res) = reject_cross_origin(&req)? {
        return Ok(res);
    }
    let Some(pid) = cookie_pid(&req)? else {
        return error(401, "missing session");
    };
    let state_json = to_json(&GameState::default());
    let row = env
        .d1("DB")?
        .prepare(
            "INSERT INTO saves (player_id, state, schema_version, version, updated_at, cafe_credit) \
             VALUES (?1, ?2, ?3, 1, ?4, ?5) \
             ON CONFLICT (player_id) DO UPDATE SET state = excluded.state, \
             schema_version = excluded.schema_version, version = saves.version + 1, \
             updated_at = excluded.updated_at, cafe_credit = excluded.cafe_credit \
             RETURNING version",
        )
        .bind(&[
            pid.as_str().into(),
            state_json.as_str().into(),
            SAVE_SCHEMA_VERSION.into(),
            now_ms().into(),
            CAFE_CREDIT_START.into(),
        ])?
        .first::<VersionRow>(None)
        .await?
        .ok_or_else(|| Error::RustError("reset returned no row".into()))?;
    reply(
        200,
        format!(r#"{{"state":{state_json},"version":{}}}"#, row.version),
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Fx {
    On,
    Off,
    Reduced,
}

#[derive(Deserialize)]
struct PerfSample {
    secs: f64,
    hz: f64,
    render_ms_avg: f64,
    render_ms_p95: f64,
    jank: f64,
    idle_frac: f64,
    fx: Fx,
    tier: f64,
    mobile: bool,
    dpr: f64,
    cores: f64,
    mem: Option<f64>,
    clicks_per_min: f64,
}

/// None when the number is not finite, so the sample is rejected.
fn clamp(v: f64, lo: f64, hi: f64) -> Option<f64> {
    v.is_finite().then(|| v.clamp(lo, hi))
}

fn ua_family(ua: &str) -> &'static str {
    let has = |s: &str| ua.contains(s);
    if has("Firefox") || has("FxiOS") {
        "firefox"
    } else if has("iPhone") || has("iPad") {
        "ios-safari"
    } else if has("Android") {
        if has("Chrome") && !has("Edg") && !has("OPR") {
            "android-chrome"
        } else {
            "other"
        }
    } else if has("Edg/") || has("OPR/") {
        "other"
    } else if has("Chrome") {
        "desktop-chrome"
    } else if has("Safari") {
        "desktop-safari"
    } else {
        "other"
    }
}

/// Anonymous aggregate telemetry: no cookie, no body echo, nothing that identifies the request.
async fn post_telemetry(mut req: Request) -> Result<Response> {
    if let Some(res) = reject_cross_origin(&req)? {
        return Ok(res);
    }
    let Some(bytes) = read_limited(&mut req, MAX_TELEMETRY_BYTES).await? else {
        return error(413, "body too large");
    };
    let Ok(s) = serde_json::from_slice::<PerfSample>(&bytes) else {
        return error(400, "malformed body");
    };
    let mem = match s.mem {
        Some(m) => match clamp(m, 0.0, 512.0) {
            Some(m) => Some(m),
            None => return error(400, "malformed body"),
        },
        None => None,
    };
    let fields = (
        clamp(s.secs, 0.0, 3600.0),
        clamp(s.hz, 0.0, 240.0),
        clamp(s.render_ms_avg, 0.0, 1000.0),
        clamp(s.render_ms_p95, 0.0, 1000.0),
        clamp(s.jank, 0.0, 100_000.0),
        clamp(s.idle_frac, 0.0, 1.0),
        clamp(s.tier, 0.0, 5.0),
        clamp(s.dpr, 0.0, 10.0),
        clamp(s.cores, 0.0, 256.0),
        clamp(s.clicks_per_min, 0.0, 1000.0),
    );
    let (
        Some(secs),
        Some(hz),
        Some(avg),
        Some(p95),
        Some(jank),
        Some(idle_frac),
        Some(tier),
        Some(dpr),
        Some(cores),
        Some(cpm),
    ) = fields
    else {
        return error(400, "malformed body");
    };
    let ua = req.headers().get("User-Agent")?.unwrap_or_default();
    let country = req.cf().and_then(|cf| cf.country());
    console_log!(
        "{}",
        json!({
            "event": "perf",
            "v": 1,
            "secs": secs,
            "hz": hz,
            "render_ms_avg": avg,
            "render_ms_p95": p95,
            "jank": jank.round(),
            "idle_frac": idle_frac,
            "fx": match s.fx { Fx::On => "on", Fx::Off => "off", Fx::Reduced => "reduced" },
            "tier": tier.round(),
            "mobile": s.mobile,
            "dpr": dpr,
            "cores": cores.round(),
            "mem": mem,
            "clicks_per_min": cpm,
            "country": country,
            "ua": ua_family(&ua),
        })
    );
    Ok(Response::empty()?.with_status(204))
}

async fn route(req: Request, env: Env) -> Result<Response> {
    let path = req.path();
    let method = req.method();
    let allowed: &[Method] = match path.as_str() {
        "/api/save" => &[Method::Get, Method::Put],
        "/api/reset" | "/api/telemetry" => &[Method::Post],
        _ => return error(404, "not found"),
    };
    if !allowed.contains(&method) {
        let mut res = error(405, "method not allowed")?;
        let list: Vec<_> = allowed.iter().map(|m| m.to_string()).collect();
        res.headers_mut().set("Allow", &list.join(", "))?;
        return Ok(res);
    }
    match (path.as_str(), method) {
        ("/api/save", Method::Get) => get_save(req, env).await,
        ("/api/save", _) => put_save(req, env).await,
        ("/api/telemetry", _) => post_telemetry(req).await,
        _ => reset(req, env).await,
    }
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    match route(req, env).await {
        Ok(res) => Ok(res),
        Err(e) => {
            console_error!("{e}");
            error(500, "internal error")
        }
    }
}
