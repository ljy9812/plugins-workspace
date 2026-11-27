// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Expose your apps assets through a localhost server instead of the default custom protocol.
//!
//! **Note: This plugins brings considerable security risks and you should only use it if you know what your are doing. If in doubt, use the default custom protocol implementation.**

#![doc(
    html_logo_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png",
    html_favicon_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png"
)]

use std::collections::HashMap;

use http::Uri;
use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    Runtime,
};
use tiny_http::{Header, Response as HttpResponse, Server};

pub struct Request {
    url: String,
}

impl Request {
    pub fn url(&self) -> &str {
        &self.url
    }
}

pub struct Response {
    headers: HashMap<String, String>,
}

impl Response {
    pub fn add_header<H: Into<String>, V: Into<String>>(&mut self, header: H, value: V) {
        self.headers.insert(header.into(), value.into());
    }
}

type OnRequest = Option<Box<dyn Fn(&Request, &mut Response) + Send + Sync>>;

pub struct Builder {
    port: u16,
    host: Option<String>,
    on_request: OnRequest,
}

impl Builder {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            host: None,
            on_request: None,
        }
    }

    // Change the host the plugin binds to. Defaults to `localhost`.
    pub fn host<H: Into<String>>(mut self, host: H) -> Self {
        self.host = Some(host.into());
        self
    }

    pub fn on_request<F: Fn(&Request, &mut Response) + Send + Sync + 'static>(
        mut self,
        f: F,
    ) -> Self {
        self.on_request.replace(Box::new(f));
        self
    }

    pub fn build<R: Runtime>(mut self) -> TauriPlugin<R> {
        let port = self.port;
        // OHOS keeps the raw Option so it can warn when a custom host is
        // ignored; other platforms keep the original resolved String.
        #[cfg(target_env = "ohos")]
        let host = self.host;
        #[cfg(not(target_env = "ohos"))]
        let host = self.host.unwrap_or("localhost".to_string());
        let on_request = self.on_request.take();

        PluginBuilder::new("localhost")
            .setup(move |app, _api| {
                let asset_resolver = app.asset_resolver();

                // OHOS: the device resolver only maps loopback names in
                // /etc/hosts, and `localhost` itself may resolve to ::1 only
                // on OHOS musl (see openspec 2026-07-21-p1-localhost-ohos-fix),
                // so the asset server always serves from the loopback
                // address. A custom host is ignored with a warning instead
                // of silently.
                #[cfg(target_env = "ohos")]
                if let Some(h) = host.as_deref() {
                    if h != "localhost" && h != "127.0.0.1" {
                        log::warn!(
                            "[localhost plugin] OHOS serves from 127.0.0.1 only; ignoring custom host `{}`",
                            h
                        );
                    }
                }
                // OHOS: bind synchronously here so a failure fails app setup
                // (surfaces as Error::PluginInitialization) — a silently dead
                // asset server is far harder to diagnose than a failed start.
                // Binding is a fast syscall; the accept loop still runs on
                // the spawned thread below.
                //
                // OHOS retry carve-out: the AMS restart race can relaunch a
                // second instance while the dying predecessor still holds the
                // port (EADDRINUSE / os error 98) — its sockets close once the
                // process is reaped, typically within seconds. Retrying only
                // AddrInUse with a 250ms backoff rides that race out. The
                // failure OUTCOME is unchanged (any other bind error, or a
                // port still occupied after the 10s deadline, fails setup),
                // but the timing is not: every AddrInUse retries ~40 times
                // over the deadline, so a genuinely squatted port also waits
                // out the full window before the failure surfaces — this is
                // NOT fail-fast for that case. The retry loop runs
                // synchronously on the OHOS startup main thread (it has to,
                // to be able to fail setup), an accepted startup-blocking
                // tradeoff: the common case binds immediately, the race case
                // frees the port within seconds, and only a real squatter
                // pays the full 10s (an appfreeze/watchdog risk worth its
                // rarity — failing the relaunch outright would instead break
                // every AMS restart).
                #[cfg(target_env = "ohos")]
                let bind_addr = format!("127.0.0.1:{}", port);
                #[cfg(target_env = "ohos")]
                let server = {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                    loop {
                        match Server::http(&bind_addr) {
                            Ok(s) => {
                                log::info!("[localhost plugin] listening on {}", bind_addr);
                                break s;
                            }
                            // tiny_http::Server::http returns Box<dyn Error>;
                            // the bind failure is an std::io::Error boxed
                            // inside it (os error 98 = EADDRINUSE), so the
                            // transient-race check goes through a downcast.
                            Err(e) if e
                                .downcast_ref::<std::io::Error>()
                                .map(|io| io.kind() == std::io::ErrorKind::AddrInUse)
                                .unwrap_or(false) =>
                            {
                                if std::time::Instant::now() >= deadline {
                                    return Err(format!(
                                        "[localhost plugin] failed to bind {}: {} (still occupied after 10s — the AMS restart race should have cleared by now; check whether another process squats the port)",
                                        bind_addr, e
                                    )
                                    .into());
                                }
                                // AddrInUse alone cannot distinguish the
                                // restart race from a squatter — do not
                                // attribute; the deadline message above
                                // carries the diagnosis.
                                log::warn!(
                                    "[localhost plugin] bind failed with AddrInUse on {}; retrying",
                                    bind_addr
                                );
                                std::thread::sleep(std::time::Duration::from_millis(250));
                            }
                            Err(e) => {
                                return Err(format!(
                                    "[localhost plugin] failed to bind {}: {}",
                                    bind_addr, e
                                )
                                .into());
                            }
                        }
                    }
                };

                std::thread::spawn(move || {
                    #[cfg(not(target_env = "ohos"))]
                    let bind_addr = format!("{}:{}", host, port);
                    #[cfg(not(target_env = "ohos"))]
                    let server = Server::http(&bind_addr).expect("Unable to spawn server");
                    for req in server.incoming_requests() {
                        let path = req
                            .url()
                            .parse::<Uri>()
                            .map(|uri| uri.path().into())
                            .unwrap_or_else(|_| req.url().into());

                        #[allow(unused_mut)]
                        if let Some(mut asset) = asset_resolver.get(path) {
                            let request = Request {
                                url: req.url().into(),
                            };
                            let mut response = Response {
                                headers: Default::default(),
                            };

                            response.add_header("Content-Type", asset.mime_type);
                            if let Some(csp) = asset.csp_header {
                                response
                                    .headers
                                    .insert("Content-Security-Policy".into(), csp);
                            }

                            response
                                .headers
                                .insert("Cache-Control".into(), "no-cache".into());

                            // CORS: OHOS-only. ArkWeb CspBypassing makes CSP unreliable on OHOS,
                            // and the examples/api test fetches the localhost server cross-origin
                            // (tauri://localhost page -> http://127.0.0.1:PORT). Gate to OHOS so
                            // non-OHOS platforms keep their existing (stricter) CORS posture.
                            #[cfg(target_env = "ohos")]
                            {
                                response.headers.insert(
                                    "Access-Control-Allow-Origin".into(),
                                    "*".into(),
                                );
                                response.headers.insert(
                                    "Access-Control-Allow-Methods".into(),
                                    "GET, POST, PUT, DELETE, OPTIONS".into(),
                                );
                                response.headers.insert(
                                    "Access-Control-Allow-Headers".into(),
                                    "*".into(),
                                );
                            }

                            if let Some(on_request) = &on_request {
                                on_request(&request, &mut response);
                            }

                            let mut resp = HttpResponse::from_data(asset.bytes);
                            for (header, value) in response.headers {
                                if let Ok(h) = Header::from_bytes(header.as_bytes(), value) {
                                    resp.add_header(h);
                                }
                            }
                            req.respond(resp).expect("unable to setup response");
                        }
                    }
                });
                Ok(())
            })
            .build()
    }
}
