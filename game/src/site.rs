//! The game as the site's board. In the browser the page (src/client/game.ts)
//! owns the room: it polls the Worker, sends actions, and exposes
//! `window.emberSite` for this module to read through `emscripten_run_script`.
//! The Worker validates every action, so this side only shows what it is given
//! and submits what the player picks. On desktop there is no site and every
//! function here is inert.

use reach_rules::Action;

/// What the Worker last told this player about the room.
#[derive(Clone, Debug, Default)]
pub struct Snap {
    pub code: String,
    pub revision: i64,
    /// This player's seat.
    pub you: usize,
    pub finished: bool,
    /// Seat names and whether the Worker plays them.
    pub seats: Vec<(String, bool)>,
    /// The rules state with the other hands hidden.
    pub state: Vec<i64>,
    /// The last events, oldest first, and how many actions have been played in all.
    pub log: Vec<[i64; 5]>,
    pub log_total: i64,
    /// Set when the Worker refused the last submitted action.
    pub error: Option<String>,
}

#[cfg(target_os = "emscripten")]
mod imp {
    use super::*;
    use std::ffi::{c_char, CStr, CString};

    extern "C" {
        fn emscripten_run_script_string(script: *const c_char) -> *const c_char;
    }

    fn run(js: &str) -> String {
        let Ok(c) = CString::new(js) else { return String::new() };
        unsafe {
            let p = emscripten_run_script_string(c.as_ptr());
            if p.is_null() {
                String::new()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }

    pub fn enabled() -> bool {
        std::env::args().any(|a| a == "--site")
    }

    pub fn take() -> Option<Snap> {
        let raw = run("window.emberSite ? window.emberSite.take() : ''");
        if raw.is_empty() {
            return None;
        }
        parse(&raw)
    }

    pub fn submit(a: Action) {
        run(&format!("window.emberSite.submit([{},{},{},{}]); ''", a.0[0], a.0[1], a.0[2], a.0[3]));
    }

    pub fn leave() {
        run("window.emberSite.leave(); ''");
    }

    /// The board and its sky are built: the page can take its loading veil down.
    pub fn ready() {
        run("if (window.emberBoardReady) window.emberBoardReady(); ''");
    }

    /// Publish the frame probe as a JS global and a hidden DOM node. The page's
    /// probe server receives the same JSON so the assertions read one snapshot.
    pub fn export_probe(json: &str, ready: bool) {
        let lit = serde_json::to_string(json).unwrap_or_else(|_| "\"{}\"".into());
        let js = format!(
            "window.__reachProbe = JSON.parse({lit}); window.__reachReady = {ready};\
             var p=document.getElementById('reach-probe');\
             if(!p){{p=document.createElement('pre');p.id='reach-probe';\
             p.style.cssText='position:fixed;left:-10000px;top:0;width:1px;height:1px;overflow:hidden';\
             document.body.appendChild(p);}}\
             p.textContent = JSON.stringify(window.__reachProbe);\
             if (window.__reachProbeName) {{\
               fetch('/probe?name=' + encodeURIComponent(window.__reachProbeName),\
                 {{method:'POST', body: JSON.stringify(window.__reachProbe)}});\
             }}\n+             ''"
        );
        run(&js);
    }
}

#[cfg(not(target_os = "emscripten"))]
mod imp {
    use super::*;
    pub fn enabled() -> bool {
        false
    }
    pub fn take() -> Option<Snap> {
        None
    }
    pub fn submit(_: Action) {}
    pub fn leave() {}
    pub fn ready() {}
    pub fn export_probe(_: &str, _: bool) {}
}

pub use imp::{enabled, export_probe, leave, ready, submit, take};

/// Reads the snapshot JSON the page builds (see `emberSite.take` in game.ts).
pub fn parse(raw: &str) -> Option<Snap> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let ints = |x: &serde_json::Value| -> Vec<i64> { x.as_array().map(|a| a.iter().filter_map(|n| n.as_i64()).collect()).unwrap_or_default() };
    let state = ints(&v["state"]);
    if state.is_empty() {
        return None;
    }
    let log = v["log"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|e| {
                    let e = ints(e);
                    let mut out = [0i64; 5];
                    for (i, x) in e.iter().take(5).enumerate() {
                        out[i] = *x;
                    }
                    out
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Snap {
        code: v["code"].as_str().unwrap_or("").to_string(),
        revision: v["revision"].as_i64().unwrap_or(0),
        you: v["you"].as_u64().unwrap_or(0) as usize,
        finished: v["finished"].as_bool().unwrap_or(false),
        seats: v["seats"]
            .as_array()
            .map(|a| a.iter().map(|s| (s["name"].as_str().unwrap_or("?").to_string(), s["bot"].as_bool().unwrap_or(false))).collect())
            .unwrap_or_default(),
        state,
        log,
        log_total: v["logTotal"].as_i64().unwrap_or(0),
        error: v["error"].as_str().map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_snapshot() {
        let raw = r#"{"code":"MIRA","revision":7,"you":1,"finished":false,
            "seats":[{"name":"Ana","bot":false},{"name":"Bot 1","bot":true}],
            "state":[2,1,0,1],"log":[[0,1,3,-1,0],[4,0,0,0,0]],"logTotal":9,"error":"stale"}"#;
        let s = parse(raw).unwrap();
        assert_eq!((s.code.as_str(), s.revision, s.you, s.log_total), ("MIRA", 7, 1, 9));
        assert_eq!(s.seats[1], ("Bot 1".to_string(), true));
        assert_eq!(s.log[0], [0, 1, 3, -1, 0]);
        assert_eq!(s.error.as_deref(), Some("stale"));
        assert!(parse(r#"{"state":[]}"#).is_none());
    }
}
