//! Linus Browser — a Brave-style browser built on GTK4 + WebKitGTK.
//!
//! Features: tab strip, Shields (ad/tracker blocking, HTTPS upgrade, third-party
//! cookie blocking, per-site switch), persistent bookmarks + bookmarks bar,
//! history, downloads, private windows, find in page, zoom, dev tools,
//! fullscreen, a New Tab page with top sites, and keyboard shortcuts.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Entry,
    EntryIconPosition, GestureClick, Image, Label, MenuButton, Orientation, Popover, ProgressBar,
    STYLE_PROVIDER_PRIORITY_USER, SearchEntry, Stack, Switch, WindowHandle, gdk, gio,
    glib, pango, prelude::*, style_context_add_provider_for_display,
};
use webkit6::{
    CookieAcceptPolicy, FindOptions, LoadEvent, NavigationPolicyDecision, NetworkSession,
    PolicyDecisionType, TLSErrorsPolicy, UserContentFilter, UserContentFilterStore,
    UserContentInjectedFrames, UserContentManager, UserScript, UserScriptInjectionTime, WebView,
    prelude::*,
};

const TUX_PNG: &[u8] = include_bytes!("assets/tux.png");
const SETTINGS_GEAR_PNG: &[u8] = include_bytes!("assets/settings-gear.png");
const DEFAULT_NEW_TAB_BACKGROUND: &[u8] = include_bytes!("assets/brave-red-wallpaper.png");
const DUCKDUCKGO_SEARCH_ICON: &[u8] = include_bytes!("assets/search-duckduckgo.ico");
const GOOGLE_SEARCH_ICON: &[u8] = include_bytes!("assets/search-google.png");
const BRAVE_SEARCH_ICON: &[u8] = include_bytes!("assets/search-brave.svg");
const APP_TITLE: &str = "Linus Browser";
const FINGERPRINT_STATUS_ACTIVE: &str = "Canvas/WebGL farbling active (partial)";
const FINGERPRINT_STATUS_INACTIVE: &str = "Fingerprint randomization: off";
const FINGERPRINT_STATUS_UNAVAILABLE: &str = "Fingerprint randomization unavailable";
const FINGERPRINT_INFO: &str = "Best-effort, session- and site-specific noise is applied to canvas/WebGL readbacks and WebGL renderer details are masked. Other signals remain exposed. WebGL and WebRTC stay enabled. This does not reproduce Brave's engine-level protection or guarantee a Cover Your Tracks result.";
const FINGERPRINT_FARBLING_SCRIPT: &str = r#"
(() => {
  if (window.__linusFingerprintFarbling) return;
  Object.defineProperty(window, '__linusFingerprintFarbling', { value: true });

  const seed = '__SESSION_SEED__|' + location.hostname.toLowerCase();
  let state = 2166136261;
  for (let i = 0; i < seed.length; i++) {
    state = Math.imul(state ^ seed.charCodeAt(i), 16777619);
  }
  const noise = (index) => {
    let value = state ^ Math.imul(index + 1, 0x45d9f3b);
    value = Math.imul(value ^ (value >>> 16), 0x45d9f3b);
    value = Math.imul(value ^ (value >>> 16), 0x45d9f3b);
    return (value ^ (value >>> 16)) >>> 0;
  };
  const farblePixels = (pixels) => {
    for (let i = 0; i < pixels.length; i += 4) {
      const value = noise(i >>> 2);
      if ((value & 7) === 0) {
        const channel = (value >>> 3) % 3;
        pixels[i + channel] = (pixels[i + channel] + ((value >>> 5) & 1 ? 1 : 255)) & 255;
      }
    }
  };

  const context2d = window.CanvasRenderingContext2D &&
    CanvasRenderingContext2D.prototype;
  if (context2d) {
    const nativeGetImageData = context2d.getImageData;
    const nativePutImageData = context2d.putImageData;
    const nativeToDataURL = HTMLCanvasElement.prototype.toDataURL;
    const nativeToBlob = HTMLCanvasElement.prototype.toBlob;
    context2d.getImageData = function (...args) {
      const image = nativeGetImageData.apply(this, args);
      farblePixels(image.data);
      return image;
    };
    const farbledCanvas = (canvas) => {
      const copy = document.createElement('canvas');
      copy.width = canvas.width;
      copy.height = canvas.height;
      const context = copy.getContext('2d', { willReadFrequently: true });
      if (!context) return canvas;
      try {
        context.drawImage(canvas, 0, 0);
        const image = nativeGetImageData.call(context, 0, 0, copy.width, copy.height);
        farblePixels(image.data);
        nativePutImageData.call(context, image, 0, 0);
      } catch (_) {
        return canvas;
      }
      return copy;
    };
    HTMLCanvasElement.prototype.toDataURL = function (...args) {
      return nativeToDataURL.apply(farbledCanvas(this), args);
    };
    HTMLCanvasElement.prototype.toBlob = function (callback, ...args) {
      return nativeToBlob.call(farbledCanvas(this), callback, ...args);
    };
  }

  const offscreen = window.OffscreenCanvas && OffscreenCanvas.prototype;
  if (context2d && offscreen && typeof offscreen.convertToBlob === 'function') {
    const nativeConvertToBlob = offscreen.convertToBlob;
    offscreen.convertToBlob = function (...args) {
      const copy = document.createElement('canvas');
      copy.width = this.width;
      copy.height = this.height;
      const context = copy.getContext('2d', { willReadFrequently: true });
      if (!context) return nativeConvertToBlob.apply(this, args);
      try {
        context.drawImage(this, 0, 0);
        return new Promise((resolve, reject) => {
          const options = args[0] || {};
          copy.toBlob(
            blob => blob ? resolve(blob) : reject(new Error('Canvas export failed')),
            options.type,
            options.quality
          );
        });
      } catch (_) {
        return nativeConvertToBlob.apply(this, args);
      }
    };
  }

  for (const name of ['WebGLRenderingContext', 'WebGL2RenderingContext']) {
    const prototype = window[name] && window[name].prototype;
    if (!prototype) continue;
    const nativeGetParameter = prototype.getParameter;
    const nativeGetExtension = prototype.getExtension;
    const nativeReadPixels = prototype.readPixels;
    prototype.getParameter = function (parameter) {
      if (parameter === 0x9245) return 'WebKit';
      if (parameter === 0x9246) return 'WebKit WebGL';
      return nativeGetParameter.call(this, parameter);
    };
    prototype.getExtension = function (extension) {
      if (String(extension).toLowerCase() === 'webgl_debug_renderer_info') return null;
      return nativeGetExtension.call(this, extension);
    };
    prototype.readPixels = function (...args) {
      const result = nativeReadPixels.apply(this, args);
      const pixels = args[6];
      if (pixels instanceof Uint8Array || pixels instanceof Uint8ClampedArray) {
        for (let i = 0; i < pixels.length; i += 64) {
          pixels[i] = (pixels[i] + (noise(i) & 1)) & 255;
        }
      }
      return result;
    };
  }
})();
"#;

fn fingerprint_farbling_script(seed: &str) -> UserScript {
    let source = FINGERPRINT_FARBLING_SCRIPT.replace("__SESSION_SEED__", seed);
    UserScript::new(
        &source,
        UserContentInjectedFrames::AllFrames,
        UserScriptInjectionTime::Start,
        &[],
        &[],
    )
}

fn new_fingerprint_seed() -> Result<String, getrandom::Error> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

const AD_CLEANER_SCRIPT: &str = r#"
(() => {
  if (window.__linusAdCleanerInstalled) return;
  window.__linusAdCleanerInstalled = true;

  const style = document.createElement('style');
  style.textContent = `
    ins.adsbygoogle,
    [data-ad-client],
    [data-ad-slot],
    [data-advertisement],
    [data-sponsored="true"],
    [aria-label="Advertisement"],
    [aria-label="Sponsored"],
    .ad,
    .ads,
    .ad-banner,
    .ad-container,
    .ad-wrapper,
    .advertisement,
    .advert,
    .sponsored-content,
    .sponsored-post,
    .promoted-post,
    ytd-ad-slot-renderer,
    ytd-in-feed-ad-layout-renderer,
    ytd-display-ad-renderer,
    ytd-promoted-sparkles-web-renderer,
    ytd-promoted-video-renderer,
    ytd-video-ad-renderer,
    #masthead-ad,
    .ytp-ad-overlay-container,
    iframe[src*="doubleclick.net"],
    iframe[src*="googlesyndication.com"],
    iframe[src*="/adserver"],
    iframe[src*="/ads/"] {
      display: none !important;
    }
  `;
  const attachStyle = () => {
    const root = document.head || document.documentElement;
    if (root && !style.isConnected) root.appendChild(style);
  };
  attachStyle();
  if (!style.isConnected) document.addEventListener('DOMContentLoaded', attachStyle, { once: true });

  if (!/(^|\.)youtube(-nocookie)?\.com$/i.test(location.hostname)) return;

  const skipVideoAd = () => {
    const player = document.querySelector('.html5-video-player.ad-showing');
    if (!player) return;

    const skip = player.querySelector(
      '.ytp-ad-skip-button, .ytp-skip-ad-button, .ytp-ad-skip-button-modern'
    );
    if (skip && !skip.disabled) skip.click();

    const video = player.querySelector('video');
    if (video && Number.isFinite(video.duration) && video.duration > 0 &&
        video.currentTime < video.duration - 0.5) {
      try { video.currentTime = video.duration; } catch (_) {}
    }
  };
  setInterval(skipVideoAd, 750);
})();
"#;

// ───────────────────────────── persistence ─────────────────────────────

fn data_dir() -> PathBuf {
    let d = glib::user_config_dir().join("linus-browser");
    let _ = fs::create_dir_all(&d);
    d
}

fn read_lines(name: &str) -> Vec<String> {
    fs::read_to_string(data_dir().join(name))
        .map(|s| s.lines().map(String::from).collect())
        .unwrap_or_default()
}

fn write_lines(name: &str, lines: &[String]) {
    let _ = fs::write(data_dir().join(name), lines.join("\n"));
}

fn append_line(name: &str, line: &str) {
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir().join(name))
    {
        let _ = writeln!(f, "{line}");
    }
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

#[derive(Clone)]
struct Bookmark {
    title: String,
    url: String,
}

fn load_bookmarks() -> Vec<Bookmark> {
    read_lines("bookmarks.tsv")
        .iter()
        .filter_map(|l| {
            let (t, u) = l.split_once('\t')?;
            Some(Bookmark {
                title: t.to_string(),
                url: u.to_string(),
            })
        })
        .collect()
}

fn save_bookmarks(list: &[Bookmark]) {
    let lines: Vec<String> = list
        .iter()
        .map(|b| format!("{}\t{}", clean(&b.title), b.url))
        .collect();
    write_lines("bookmarks.tsv", &lines);
}

struct HistoryItem {
    time: i64,
    title: String,
    url: String,
}

fn load_history() -> Vec<HistoryItem> {
    let mut items: Vec<HistoryItem> = read_lines("history.tsv")
        .iter()
        .filter_map(|l| {
            let mut p = l.splitn(3, '\t');
            let time = p.next()?.parse().ok()?;
            let title = p.next()?.to_string();
            let url = p.next()?.to_string();
            Some(HistoryItem { time, title, url })
        })
        .collect();
    let n = items.len();
    if n > 3000 {
        items.drain(0..n - 3000);
    }
    items
}

struct Prefs {
    https_upgrade: bool,
    block_cookies: bool,
    bookmarks_bar: bool,
    search_engine: String,
    background_images: bool,
    wallpaper_style: String,
    fingerprint_farbling: bool,
    shields_off: HashSet<String>,
}

impl Prefs {
    fn load() -> Self {
        let mut p = Prefs {
            https_upgrade: true,
            block_cookies: true,
            bookmarks_bar: true,
            search_engine: "duckduckgo".to_string(),
            background_images: true,
            wallpaper_style: "default".to_string(),
            fingerprint_farbling: true,
            shields_off: HashSet::new(),
        };
        let mut wallpaper_style_configured = false;
        for line in read_lines("prefs.conf") {
            if let Some((k, v)) = line.split_once('=') {
                match k {
                    "https_upgrade" => p.https_upgrade = v == "1",
                    "block_cookies" => p.block_cookies = v == "1",
                    "bookmarks_bar" => p.bookmarks_bar = v == "1",
                    "search_engine" if matches!(v, "duckduckgo" | "google" | "brave") => {
                        p.search_engine = v.to_string()
                    }
                    "background_images" => p.background_images = v == "1",
                    "wallpaper_style"
                        if matches!(v, "default" | "custom" | "solid" | "gradient") =>
                    {
                        p.wallpaper_style = v.to_string();
                        wallpaper_style_configured = true;
                    }
                    "reduce_fingerprinting_surface" | "fingerprint_farbling" => {
                        p.fingerprint_farbling = v == "1"
                    }
                    "shields_off" => {
                        p.shields_off = v
                            .split(',')
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect()
                    }
                    _ => {}
                }
            }
        }
        if !wallpaper_style_configured && data_dir().join("new-tab-background").is_file() {
            p.wallpaper_style = "custom".to_string();
        }
        p
    }

    fn save(&self) {
        let mut off: Vec<&str> = self.shields_off.iter().map(|s| s.as_str()).collect();
        off.sort();
        let b = |v: bool| if v { "1" } else { "0" };
        write_lines(
            "prefs.conf",
            &[
                format!("https_upgrade={}", b(self.https_upgrade)),
                format!("block_cookies={}", b(self.block_cookies)),
                format!("bookmarks_bar={}", b(self.bookmarks_bar)),
                format!("search_engine={}", self.search_engine),
                format!("background_images={}", b(self.background_images)),
                format!("wallpaper_style={}", self.wallpaper_style),
                format!("fingerprint_farbling={}", b(self.fingerprint_farbling)),
                format!("shields_off={}", off.join(",")),
            ],
        );
    }
}

fn effective_wallpaper_style(style: &str, has_custom_background: bool) -> &str {
    if style == "custom" && !has_custom_background {
        "default"
    } else {
        style
    }
}

// ───────────────────────────── shields data ─────────────────────────────

/// Third-party ad / tracker domains blocked while Shields are up.
const TRACKER_DOMAINS: &[&str] = &[
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "google-analytics.com",
    "googletagmanager.com",
    "googletagservices.com",
    "adservice.google.com",
    "2mdn.net",
    "connect.facebook.net",
    "facebook.net",
    "scorecardresearch.com",
    "quantserve.com",
    "adnxs.com",
    "criteo.com",
    "criteo.net",
    "taboola.com",
    "outbrain.com",
    "hotjar.com",
    "mixpanel.com",
    "amplitude.com",
    "moatads.com",
    "adsrvr.org",
    "rubiconproject.com",
    "pubmatic.com",
    "openx.net",
    "casalemedia.com",
    "bat.bing.com",
    "clarity.ms",
    "ads-twitter.com",
    "analytics.twitter.com",
    "ads.linkedin.com",
    "snap.licdn.com",
    "analytics.tiktok.com",
    "mc.yandex.ru",
    "chartbeat.com",
    "amazon-adsystem.com",
    "adform.net",
    "bluekai.com",
    "demdex.net",
    "krxd.net",
    "mathtag.com",
    "everesttech.net",
    "yieldmo.com",
    "smartadserver.com",
    "adition.com",
    "adcolony.com",
    "adkernel.com",
    "adlooxtracking.com",
    "adroll.com",
    "adsafeprotected.com",
    "adskeeper.co.uk",
    "advertising.com",
    "advertising.microsoft.com",
    "advertising.yahoo.com",
    "affiliatewindow.com",
    "appnexus.com",
    "bidswitch.net",
    "contextweb.com",
    "de17a.com",
    "districtm.io",
    "lijit.com",
    "media.net",
    "ml314.com",
    "nativo.com",
    "omnitagjs.com",
    "onetag-sys.com",
    "perfectmarket.com",
    "pixel.rubiconproject.com",
    "popads.net",
    "propellerads.com",
    "revcontent.com",
    "sovrn.com",
    "spotxchange.com",
    "teads.tv",
    "triplelift.com",
    "undertone.com",
    "zedo.com",
    "zemanta.com",
    "adtrafficquality.google",
    "pagead2.googlesyndication.com",
    "securepubads.g.doubleclick.net",
    "adsymptotic.com",
    "adzerk.net",
    "bannersnack.com",
    "exelator.com",
    "gumgum.com",
    "improvedigital.com",
    "indexww.com",
    "inmobi.com",
    "loopme.me",
    "onetag.com",
    "sharethrough.com",
    "smaato.net",
    "sonobi.com",
    "sponsorpay.com",
    "vungle.com",
    "w55c.net",
    "yldbt.com",
];

const FILTER_LISTS: &[(&str, &str)] = &[
    ("easylist.txt", "https://easylist.to/easylist/easylist.txt"),
    (
        "easyprivacy.txt",
        "https://easylist.to/easylist/easyprivacy.txt",
    ),
];
const FILTER_LIST_REFRESH_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const FILTER_LIST_UPDATED_FILE: &str = "updated-at";

#[derive(Default)]
struct FilterListDomains {
    blocked: Vec<String>,
    allowed: Vec<String>,
}

fn parse_filter_list_domains(contents: &str) -> FilterListDomains {
    let mut blocked = HashSet::new();
    let mut allowed = HashSet::new();

    for line in contents.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('!') || line.starts_with('[') {
            continue;
        }

        let is_exception = line.starts_with("@@||");
        let pattern = if is_exception { &line[2..] } else { line };
        let (pattern, options) = pattern
            .split_once('$')
            .map_or((pattern, ""), |(pattern, options)| (pattern, options));
        if options.split(',').any(|option| {
            let option = option.trim();
            option.starts_with("domain=")
                || option.starts_with('~')
                || option == "document"
                || option == "popup"
                || option == "badfilter"
                || option.contains('=')
        }) {
            continue;
        }

        let Some(host) = pattern.strip_prefix("||") else {
            continue;
        };
        let host = host.strip_suffix('^').unwrap_or(host);
        if host.is_empty()
            || !host.contains('.')
            || host.starts_with('.')
            || host.ends_with('.')
            || host.contains("..")
            || !host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        {
            continue;
        }

        let host = host.to_ascii_lowercase();
        if is_exception {
            allowed.insert(host);
        } else {
            blocked.insert(host);
        }
    }

    FilterListDomains {
        blocked: blocked.into_iter().collect(),
        allowed: allowed.into_iter().collect(),
    }
}

fn cached_filter_lists(cache_dir: &Path) -> Option<FilterListDomains> {
    let mut combined = FilterListDomains::default();
    for (filename, _) in FILTER_LISTS {
        let contents = fs::read_to_string(cache_dir.join(filename)).ok()?;
        let parsed = parse_filter_list_domains(&contents);
        if parsed.blocked.len() < 20 {
            return None;
        }
        combined.blocked.extend(parsed.blocked);
        combined.allowed.extend(parsed.allowed);
    }
    combined.blocked.sort_unstable();
    combined.blocked.dedup();
    combined.allowed.sort_unstable();
    combined.allowed.dedup();
    Some(combined)
}

fn fetch_filter_list(url: &str) -> Result<String, String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build();
    let agent: ureq::Agent = config.into();
    let response = agent
        .get(url)
        .header(
            "User-Agent",
            concat!("LinusBrowser/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| format!("request failed: {error}"))?;
    response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("could not read response: {error}"))
}

fn refresh_filter_lists(cache_dir: &Path) -> Result<FilterListDomains, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_secs();
    let marker = cache_dir.join(FILTER_LIST_UPDATED_FILE);
    let cached = cached_filter_lists(cache_dir);
    if let Ok(updated_at) = fs::read_to_string(&marker)
        && let Ok(updated_at) = updated_at.trim().parse::<u64>()
        && now.saturating_sub(updated_at) < FILTER_LIST_REFRESH_INTERVAL.as_secs()
        && let Some(lists) = cached
    {
        return Ok(lists);
    }

    let update_result = (|| {
        let mut downloads = Vec::new();
        for (filename, url) in FILTER_LISTS {
            let contents = fetch_filter_list(url)
                .map_err(|error| format!("could not update {filename}: {error}"))?;
            if parse_filter_list_domains(&contents).blocked.len() < 20 {
                return Err(format!(
                    "downloaded {filename} did not contain enough valid rules"
                ));
            }
            downloads.push((filename, contents));
        }

        fs::create_dir_all(cache_dir)
            .map_err(|error| format!("could not create filter-list cache: {error}"))?;
        for (filename, contents) in downloads {
            let path = cache_dir.join(filename);
            let temporary_path = path.with_extension("txt.tmp");
            fs::write(&temporary_path, contents)
                .map_err(|error| format!("could not cache {filename}: {error}"))?;
            fs::rename(&temporary_path, &path)
                .map_err(|error| format!("could not replace cached {filename}: {error}"))?;
        }
        fs::write(marker, now.to_string())
            .map_err(|error| format!("could not record filter-list update time: {error}"))?;

        cached_filter_lists(cache_dir)
            .ok_or_else(|| "updated filter lists could not be parsed".to_string())
    })();

    match (update_result, cached) {
        (Ok(lists), _) => Ok(lists),
        (Err(error), Some(lists)) => {
            eprintln!("Filter-list update failed; retaining cached lists: {error}");
            Ok(lists)
        }
        (Err(error), None) => Err(error),
    }
}

/// WebKit rules for known third-party ad/tracker domains and site-specific ad elements.
fn filter_rules_json_with_lists(lists: &FilterListDomains) -> String {
    let mut rules: Vec<serde_json::Value> = TRACKER_DOMAINS
        .iter()
        .copied()
        .chain(lists.blocked.iter().map(String::as_str))
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|d| {
            let domain_pattern = d.replace('.', r"\.");
            serde_json::json!({
                "trigger": {
                    "url-filter": format!(r"^https?://([^/]+\.)?{}[:/]", domain_pattern),
                    "load-type": ["third-party"]
                },
                "action": {"type": "block"}
            })
        })
        .collect();
    for domain in &lists.allowed {
        let domain_pattern = domain.replace('.', r"\.");
        rules.push(serde_json::json!({
            "trigger": {
                "url-filter": format!(r"^https?://([^/]+\.)?{}[:/]", domain_pattern),
                "load-type": ["third-party"]
            },
            "action": {"type": "ignore-previous-rules"}
        }));
    }
    rules.push(serde_json::json!({
        "trigger": {
            "url-filter": r"^https?://(www\.)?youtube(-nocookie)?\.com/",
            "if-domain": ["youtube.com", "www.youtube.com", "m.youtube.com", "youtube-nocookie.com"]
        },
        "action": {
            "type": "css-display-none",
            "selector": "ytd-ad-slot-renderer, ytd-in-feed-ad-layout-renderer, ytd-display-ad-renderer, ytd-promoted-sparkles-web-renderer, ytd-promoted-video-renderer, #masthead-ad"
        }
    }));
    rules.push(serde_json::json!({
        "trigger": {
            "url-filter": r"^https?://(www\.)?duckduckgo\.com/",
            "if-domain": ["duckduckgo.com"]
        },
        "action": {
            "type": "css-display-none",
            "selector": ".results--ads, #ads, [data-testid='ad'], .js-sidebar-promo"
        }
    }));
    serde_json::to_string(&rules).expect("serializing content blocker rules cannot fail")
}

fn host_of(uri: &str) -> Option<String> {
    glib::Uri::parse(uri, glib::UriFlags::NONE)
        .ok()
        .and_then(|u| u.host())
        .map(|h| h.to_string())
}

fn is_local_host(h: &str) -> bool {
    h == "localhost" || h.ends_with(".local") || h.parse::<std::net::IpAddr>().is_ok()
}

fn apply_cookie_policy(session: Option<NetworkSession>, block: bool) {
    if let Some(cm) = session.and_then(|s| s.cookie_manager()) {
        cm.set_accept_policy(if block {
            CookieAcceptPolicy::NoThirdParty
        } else {
            CookieAcceptPolicy::Always
        });
    }
}

// ───────────────────────────── helpers ─────────────────────────────

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_history_date(timestamp: i64, format: &str) -> String {
    glib::DateTime::from_unix_local(timestamp)
        .ok()
        .and_then(|date| date.format(format).ok())
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn history_favicon_path(host: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    host.to_ascii_lowercase().hash(&mut hasher);
    data_dir()
        .join("favicons")
        .join(format!("{:016x}.png", hasher.finish()))
}

fn save_history_favicon(wv: &WebView) {
    let uri = wv.uri().map(|uri| uri.to_string()).unwrap_or_default();
    if !(uri.starts_with("http://") || uri.starts_with("https://")) {
        return;
    }
    let Some(host) = host_of(&uri) else {
        return;
    };
    let Some(texture) = wv.favicon() else {
        return;
    };
    let path = history_favicon_path(&host);
    let Some(directory) = path.parent() else {
        return;
    };
    if let Err(error) = fs::create_dir_all(directory) {
        eprintln!("Could not create history favicon directory: {error}");
        return;
    }
    if let Err(error) = texture.save_to_png(&path) {
        eprintln!("Could not cache favicon for {host}: {error}");
    }
}

fn history_favicon_src(url: &str) -> String {
    let Some(host) = host_of(url) else {
        return String::new();
    };
    let path = history_favicon_path(&host);
    match fs::read(&path) {
        Ok(bytes) => format!("data:image/png;base64,{}", glib::base64_encode(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let scheme = if url.starts_with("https://") {
                "https"
            } else {
                "http"
            };
            let parsed = glib::Uri::parse(url, glib::UriFlags::NONE).ok();
            let port = parsed
                .as_ref()
                .map(|uri| uri.port())
                .filter(|port| *port >= 0)
                .map(|port| format!(":{port}"))
                .unwrap_or_default();
            format!("{scheme}://{host}{port}/favicon.ico")
        }
        Err(error) => {
            eprintln!("Could not read cached favicon for {host}: {error}");
            String::new()
        }
    }
}

fn history_site_icon_html(favicon_url: &str) -> String {
    format!(
        r#"<span class="site-icon"><img class="site-fallback" src="data:image/png;base64,{}" alt=""><img class="site-favicon" src="{}" alt="" referrerpolicy="no-referrer" onload="this.previousElementSibling.hidden=true" onerror="this.hidden=true"></span>"#,
        glib::base64_encode(TUX_PNG),
        esc(favicon_url)
    )
}

fn search_url(engine: &str) -> &'static str {
    match engine {
        "google" => "https://www.google.com/search?q=",
        "brave" => "https://search.brave.com/search?q=",
        _ => "https://duckduckgo.com/?q=",
    }
}

fn normalize_input(input: &str, engine: &str) -> String {
    let t = input.trim();
    if t.is_empty() {
        return "about:blank".to_string();
    }
    if t.starts_with("about:") || t.starts_with("file:") {
        return t.to_string();
    }
    if let Some((scheme, rest)) = t.split_once("://")
        && (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
    {
        return format!(
            "{}://{}",
            scheme.to_ascii_lowercase(),
            rest.replace(' ', "%20")
        );
    }
    if t.contains("://") {
        return format!(
            "{}{}",
            search_url(engine),
            glib::Uri::escape_string(t, None::<&str>, true)
        );
    }
    if t.starts_with("localhost") || t.starts_with("127.0.0.1") {
        return format!("http://{t}");
    }
    if !t.contains(' ') && t.contains('.') {
        return format!("https://{t}");
    }
    format!(
        "{}{}",
        search_url(engine),
        glib::Uri::escape_string(t, None::<&str>, true)
    )
}

fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let base = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let mut path = dir.join(&base);
    let mut n = 1;
    while path.exists() {
        let p = Path::new(&base);
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = p
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        path = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    path
}

fn switch_row(text: &str) -> (GtkBox, Switch) {
    let row = GtkBox::new(Orientation::Horizontal, 16);
    let label = Label::new(Some(text));
    label.set_hexpand(true);
    label.set_xalign(0.0);
    let sw = Switch::new();
    sw.set_valign(Align::Center);
    row.append(&label);
    row.append(&sw);
    (row, sw)
}

// ───────────────────────────── styling / pages ─────────────────────────────

const CSS: &str = r#"
/* ── Design tokens ────────────────────────────────────────────────
   A single dark ramp plus one accent. Shell surfaces sit deliberately
   darker than the tab strip so the selected tab reads as "in front";
   the content area is lighter than the chrome so pages feel lifted.
   Every rule below is built from these, so a different wallpaper only
   has to swap this block. */
@define-color shell #16161a;          /* tab strip / outer shell      */
@define-color chrome #202027;         /* toolbar surface              */
@define-color surface #2a2a33;        /* raised: active tab, menus    */
@define-color surface_hi #34343f;     /* hover on raised surfaces     */
@define-color canvas #0e0e11;         /* behind page content          */
@define-color window_bg @shell;
@define-color window_bg_color @shell;

@define-color fg #ececf1;
@define-color fg_dim #9a9aa8;
@define-color fg_mute #6b6b7a;

@define-color accent_color #ff6a4d;
@define-color accent_rgb rgb(255, 106, 77);
@define-color accent_fg #ffffff;
@define-color focus_ring rgba(255, 106, 77, 0.35);

@define-color hairline rgba(255,255,255,0.07);
@define-color hairline_strong rgba(255,255,255,0.13);
@define-color hover rgba(255,255,255,0.08);
@define-color press rgba(255,255,255,0.14);

@define-color popover_bg @surface;
@define-color danger #ff5b6e;
@define-color success #48c78e;
@define-color warning #ffc107;

/* The address bar is the one bright surface in the chrome. */
@define-color field #ffffff;
@define-color field_border transparent;
@define-color field_fg #17171c;
@define-color field_mute #6b7280;

window, window.background { background-color: @window_bg; color: @fg; }

/* Kill the themed red "decoration" ring some GTK themes (e.g. Adwaita on
   Cinnamon) paint around frames — we draw our own hairlines instead. */
window.csd { border: none; box-shadow: none; outline: none; }
window.csd > decoration { border: none; box-shadow: none; margin: 0; padding: 0; }

/* Thin 1px outline instead of GTK's default chunky focus boxes. */
*:focus-visible { outline: none; }

/* ── Tab strip ───────────────────────────────────────────────────
   Tabs are flattened: no borders, no rings. The active tab is the only
   raised surface, which is exactly how Brave/Chrome signal "selected". */
.tabbar {
    background: @shell;
    padding: 0 8px;
    min-height: 40px;
}
/* A WindowHandle adds its own padding; drop it so the strip sits flush. */
.tabbar-handle { padding: 0; margin: 0; }
/* The tab strip controls must not paint a raised box; only the tabs get a
   surface. Without this GTK's default button background shows through as a
   lighter block at the end of the strip. */
.tabbar button.tabbar-button, .tabbar button.tabbar-close,
.tabbar button.tabbar-button:hover, .tabbar button.tabbar-close:hover {
    background-image: none; box-shadow: none; outline: none;
}
.tabbar button.tabbar-button, .tabbar button.tabbar-close {
    background-color: transparent; color: @fg_mute; border: none; border-radius: 7px;
    min-width: 26px; min-height: 26px; padding: 0; margin: 0;
    outline: none;
    transition: background 120ms ease, color 120ms ease;
}
.tabbar button.tabbar-button:hover, .tabbar button.tabbar-close:hover {
    background: @hover; color: @fg;
}
.tabbar button.tabbar-button:active, .tabbar button.tabbar-close:active {
    background: @press;
}
.tabbar button:disabled { opacity: 0.30; }

/* Keep the tab strip controls on the same baseline as the tabs themselves. */
.tabbar > box { margin: 0; }

.browser-tab {
    background: transparent; color: @fg_dim;
    border: none; border-radius: 8px;
    padding: 0 4px 0 8px; margin: 0 1px;
    min-height: 32px;
    /* Bounded width: without a cap a single tab stretched across the whole
       strip and squeezed its own label. GTK CSS has no max-width, so the
       upper bound is applied as a size request in Rust. */
    min-width: 140px;
    outline: none;
    transition: background 120ms ease, color 120ms ease;
}
.browser-tab:hover { background: @hover; color: @fg; }
.browser-tab.active {
    background: @surface; color: @fg;
    box-shadow: 0 1px 3px rgba(0,0,0,0.45);
}
.browser-tab.active label { font-weight: 600; }
.browser-tab:focus-visible { outline: 1px solid @accent_color; }

.browser-tab button {
    background: transparent; border: none; border-radius: 5px;
    min-width: 18px; min-height: 18px; padding: 0; color: inherit;
    opacity: 0; outline: none;
    transition: opacity 120ms ease, background 120ms ease;
}
/* Close button only appears on hover or for the selected tab. */
.browser-tab:hover button, .browser-tab.active button { opacity: 0.7; }
.browser-tab button:hover { background: @press; opacity: 1; }

/* ── Toolbar ──────────────────────────────────────────────────── */
.toolbar-row {
    background: @chrome;
    padding: 6px 10px;
    border-bottom: 1px solid @hairline;
}
/* A plain box still inherits Adwaita's background-image/box-shadow, which
   painted a lighter block behind the action icons. Clear all of them. */
.toolbar-group, .toolbar-group:dir(ltr) {
    background: none; background-image: none; box-shadow: none; border: none;
    margin: 0; padding: 0;
}
.toolbar-group > button, .toolbar-group > menubutton { margin: 0 1px; }
.toolbar-row button, .toolbar-row menubutton > button {
    background: transparent; color: @fg; border: none; border-radius: 7px;
    min-width: 32px; min-height: 32px; padding: 0;
    outline: none;
    transition: background 120ms ease, color 120ms ease, opacity 120ms ease;
}
/* Symbolic icons inherit the button color; force it so arrows stay legible. */
.toolbar-row button image, .toolbar-row menubutton > button image { color: @fg; }
.toolbar-row button:hover image, .toolbar-row menubutton > button:hover image { color: #ffffff; }
.toolbar-row button:hover, .toolbar-row menubutton > button:hover {
    background: @hover; color: @fg;
}
.toolbar-row button:active, .toolbar-row menubutton > button:active { background: @press; }
/* Disabled navigation stays clearly visible (just muted), instead of
   disappearing into the dark toolbar. */
.toolbar-row button:disabled, .toolbar-row menubutton > button:disabled { opacity: 0.42; }
.toolbar-row button:disabled image { color: @fg_mute; }
.toolbar-row button:focus-visible,
.toolbar-row menubutton > button:focus-visible { outline: 1px solid @accent_color; }

/* ── Address bar ────────────────────────────────────────────────
   White pill, no border, no outer ring: the shape and contrast do the
   work. Focus only lifts it very slightly, so typing never produces the
   stacked double-outline look. */
entry.url-entry {
    background: @field; color: @field_fg; caret-color: @accent_color;
    border: none; border-radius: 999px;
    padding: 0 16px; min-height: 32px;
    margin: 0 4px;
    outline: none;
    box-shadow: 0 1px 2px rgba(0,0,0,0.25);
    transition: box-shadow 120ms ease, background 120ms ease;
}
entry.url-entry:hover { background: #f7f8fa; }
entry.url-entry:focus, entry.url-entry:focus-within {
    background: #ffffff;
    outline: none;
    box-shadow: 0 2px 8px rgba(0,0,0,0.38);
}
entry.url-entry image { color: @field_mute; margin-right: 8px; }
entry.url-entry text { background: transparent; color: @field_fg; }
entry.url-entry text placeholder { color: @field_mute; opacity: 1; }
entry.url-entry selection { background: @accent_color; color: #ffffff; }

/* ── Bookmarks bar ───────────────────────────────────────────── */
.bookmarks-bar {
    background: @chrome; padding: 0 10px 6px 10px;
}
.bookmarks-bar button {
    background: transparent; color: @fg_dim; border: none; border-radius: 6px;
    padding: 3px 9px; min-height: 22px; font-size: 12px;
    outline: none;
    transition: background 120ms ease, color 120ms ease;
}
.bookmarks-bar button:hover { background: @hover; color: @fg; }
.bookmarks-bar button:active { background: @press; }

/* ── Find bar ────────────────────────────────────────────────── */
.find-bar {
    background: @chrome; padding: 7px 10px; border-bottom: 1px solid @hairline;
}
.find-bar entry, .find-bar searchentry {
    background: @surface; color: @fg;
    border: none; border-radius: 7px;
    min-height: 28px; padding: 0 12px;
}
.find-bar entry:focus-within, .find-bar searchentry:focus-within {
    background: @surface_hi; box-shadow: inset 0 0 0 1px @accent_color;
}
.find-bar button {
    background: transparent; color: @fg_dim; border: none; border-radius: 6px;
    min-width: 28px; min-height: 28px; outline: none;
}
.find-bar button:hover { background: @hover; color: @fg; }

/* ── Status / progress ───────────────────────────────────────── */
statusbar, .statusbar { background: @shell; color: @fg_mute; border-top: 1px solid @hairline; }
progressbar.load-progress trough, progressbar.load-progress progress { min-height: 2px; }
progressbar.load-progress trough { background: transparent; }
progressbar.load-progress progress {
    background: linear-gradient(90deg, @accent_color, shade(@accent_color, 1.25));
}

/* ── Popovers / menus ────────────────────────────────────────── */
popover.background, popover > contents {
    background: @popover_bg;
    color: @fg;
    border: 1px solid @hairline_strong;
    border-radius: 12px;
    box-shadow: 0 16px 40px rgba(0,0,0,0.55);
}
popover label { color: @fg; }
popover label.dim-label { color: @fg_dim; }
popover label.warning { color: @warning; }
popover label.title-4 { font-weight: 700; }
popover separator { background: @hairline; }
popover modelbutton, popover button.flat {
    border-radius: 8px; padding: 6px 10px; min-height: 30px;
    transition: background 130ms ease;
}
popover modelbutton:hover, popover button.flat:hover { background: @hover; }

switch { border-radius: 999px; }
switch:checked { background: @accent_color; }
switch slider { border-radius: 999px; }

/* ── Scrollbars ──────────────────────────────────────────────── */
scrollbar slider {
    background: rgba(255,255,255,0.22); border-radius: 999px; min-width: 6px; min-height: 6px;
}
scrollbar slider:hover { background: rgba(255,255,255,0.38); }
scrollbar slider:active { background: @accent_color; }

/* ── Dialogs ─────────────────────────────────────────────────── */
window.dialog, dialog.background, .dialog {
    background: @popover_bg; color: @fg;
    border: 1px solid @hairline_strong; border-radius: 16px;
}
dialog button, window.dialog button {
    border-radius: 9px; min-height: 32px; padding: 0 14px;
}
dialog button.suggested-action, window.dialog button.suggested-action {
    background: @accent_color; color: #fff; border: none;
}
dialog button.suggested-action:hover, window.dialog button.suggested-action:hover {
    background: shade(@accent_color, 1.12);
}

/* ── Toast / badges ──────────────────────────────────────────── */
.badge, .pill { border-radius: 999px; padding: 2px 10px; font-size: 11px; font-weight: 600; }
.badge.accent { background: @accent_color; color: #fff; }
.badge.success { background: @success; color: #0b1a13; }
.badge.danger { background: @danger; color: #fff; }

/* ── Private window ────────────────────────────────────────────
   Same ramp, shifted into violet, so the geometry and contrast stay
   identical to the normal window and only the hue changes. */
window.private { background-color: #191323; }
window.private .tabbar { background: #191323; }
window.private .toolbar-row { background: #221a30; }
window.private .bookmarks-bar { background: #221a30; }
window.private .browser-tab.active {
    background: #32284a; box-shadow: 0 1px 3px rgba(0,0,0,0.45);
}
window.private .browser-tab.active label { color: #f0ebfa; }
window.private .find-bar { background: #221a30; }
window.private .find-bar entry:focus-within,
window.private .find-bar searchentry:focus-within { box-shadow: inset 0 0 0 1px #a58bff; }
window.private .statusbar { background: #191323; color: #a99bc0; }
window.private progressbar.load-progress progress {
    background: linear-gradient(90deg, #8f6ff0, #b79bf0);
}
"#;

const NEW_TAB_HTML: &str = r##"<!doctype html>
<html><head><meta charset="utf-8"><title>New Tab</title>
<style>
  * { box-sizing: border-box; }
  html { height: 100%; }
  /* min-height (not height) keeps the wallpaper covering the whole viewport
     even when the embedding WebView reports a taller layout, which previously
     left black bands above and below the image. */
  html, body { margin: 0; width: 100%; min-height: 100vh; height: 100%;
               color: #fff; font-family: system-ui, sans-serif; overflow: hidden;
               background-color: #14111a; background-repeat: no-repeat; }
  body { background:
           linear-gradient(180deg, rgba(0,0,0,.50), rgba(0,0,0,.08) 42%, rgba(0,0,0,.62)),
           var(--wallpaper) center 53% / cover no-repeat,
           radial-gradient(ellipse 40% 55% at 50% 80%, rgba({{GLOW}},.38), rgba({{GLOW}},.12) 55%, transparent 75%),
           #000; }
  body.no-background { background: #171214; }
  body.wallpaper-solid { background: radial-gradient(ellipse at 50% 28%, rgba(84,105,255,.2), transparent 55%), linear-gradient(145deg, #10182f, #17132a 58%, #24151d); }
  body.wallpaper-gradient { background: radial-gradient(ellipse at 18% 18%, rgba(255,136,104,.34), transparent 36%), radial-gradient(ellipse at 82% 78%, rgba(130,74,255,.34), transparent 42%), linear-gradient(135deg, #1b1420, #241426 55%, #101a2d); }
  .gear { position: absolute; top: 18px; right: 22px; width: 32px; height: 32px; display: grid; place-items: center; border: 0; border-radius: 50%; background: rgba(0,0,0,.22); cursor: pointer; }
  .gear:hover { background: rgba(255,255,255,.16); }
  .gear img { width: 16px; height: 16px; opacity: .95; }
  /* Center the whole stack (search + heading + tiles) in the viewport instead
     of pinning each piece to a fixed offset, which drifted on resize. */
  .stage { position: absolute; inset: 0; display: flex; flex-direction: column;
           align-items: center; justify-content: center; gap: 14px; padding: 24px;
           pointer-events: none; }
  .stage > * { pointer-events: auto; }
  .mode { width: 100%; text-align: center; color: #e8e8ec; font-size: 12px;
          letter-spacing: .02em; text-shadow: 0 2px 10px #000; }
  #f { width: min(560px, 100%); height: 52px; padding: 0 18px;
       display: flex; align-items: center; gap: 12px;
       background: rgba(22,22,24,.82); backdrop-filter: blur(14px);
       border: 1px solid rgba(255,255,255,.16);
       box-shadow: 0 10px 30px rgba(0,0,0,.38); border-radius: 999px;
       transition: border-color .16s ease, box-shadow .16s ease; }
  #f:focus-within { border-color: rgba(255,106,77,.75);
                    box-shadow: 0 10px 30px rgba(0,0,0,.45), 0 0 0 3px rgba(255,106,77,.22); }
  #f img { width: 20px; height: 20px; }
  #q { flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: #fff; font-size: 15px; }
  #q::placeholder { color: #9d9da8; }
  .tiles { display: flex; flex-wrap: wrap; gap: 18px; justify-content: center; max-width: 720px; margin-top: 10px; }
  .tile { width: 84px; text-align: center; color: #fff; text-decoration: none; font-size: 12px; }
  .tile .ic { width: 56px; height: 56px; margin: 0 auto 6px; border-radius: 16px; font-size: 24px;
              background: rgba(22,22,24,.72); backdrop-filter: blur(12px);
              border: 1px solid rgba(255,255,255,.12);
              display: flex; align-items: center; justify-content: center;
              transition: background .16s ease, border-color .16s ease, transform .16s ease; }
  .tile:hover .ic { background: rgba(40,40,46,.9); border-color: rgba(255,255,255,.24);
                    transform: translateY(-2px); }
  .tile span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .customize-backdrop { position: fixed; z-index: 4; display: none; inset: 0;
                       background: rgba(0,0,0,.58); backdrop-filter: blur(3px); }
  .customize-backdrop.open { display: block; }
  .customize { position: fixed; z-index: 5; display: none; overflow: hidden;
               width: min(860px, calc(100vw - 32px)); height: min(600px, calc(100vh - 32px));
               top: 50%; left: 50%; transform: translate(-50%, -50%);
               background: #21191a; border: 1px solid rgba(255,255,255,.12); border-radius: 18px;
               box-shadow: 0 24px 80px rgba(0,0,0,.58); }
  .customize.open { display: flex; }
  .side { flex: 0 0 220px; padding: 26px 12px; background: #191314; border-right: 1px solid rgba(255,255,255,.07); }
  .side h1 { margin: 4px 14px 28px; color: #fff3f1; font-size: 18px; font-weight: 650; letter-spacing: .01em; }
  .nav { position: relative; width: 100%; min-height: 44px; display: flex; align-items: center; gap: 14px; padding: 11px 14px; color: #c9bdbd; text-align: left; background: transparent; border: 0; border-radius: 10px; cursor: pointer; transition: background .16s ease, color .16s ease; }
  .nav:hover { color: #fff; background: rgba(255,255,255,.06); }
  .nav.active { color: #ffb1a3; background: rgba(255,111,85,.13); }
  .nav.active::before { content: ""; position: absolute; top: 10px; bottom: 10px; left: 0; width: 3px; background: #ff9a89; border-radius: 0 3px 3px 0; }
  .content { position: relative; flex: 1; min-width: 0; padding: 48px 40px; overflow: auto; }
  .close { position: absolute; top: 14px; right: 14px; display: grid; place-items: center; width: 34px; height: 34px; color: #cdbfbe; font-size: 22px; background: transparent; border: 0; border-radius: 9px; cursor: pointer; transition: background .16s ease, color .16s ease; }
  .close:hover { color: white; background: rgba(255,255,255,.09); }
  .panel { display: none; max-width: 700px; }
  .panel.active { display: block; }
  .panel h2 { margin: 2px 0 22px; color: #fff3f1; font-size: 19px; font-weight: 650; letter-spacing: .01em; }
  .toggle-row { min-height: 56px; display: flex; align-items: center; justify-content: space-between; padding: 0 18px; color: #f0e7e6; background: #2a2021; border: 1px solid rgba(255,255,255,.07); border-radius: 12px; }
  .toggle-switch { position: relative; display: block; flex: 0 0 44px; width: 44px; height: 26px; cursor: pointer; }
  .toggle-switch input { position: absolute; inset: 0; z-index: 1; width: 100%; height: 100%; margin: 0; opacity: 0; cursor: pointer; }
  .switch-track { position: absolute; inset: 0; background: #483b3c; border: 1px solid rgba(255,255,255,.2); border-radius: 999px; transition: background .18s ease, border-color .18s ease, box-shadow .18s ease; }
  .switch-track::before { content: ""; position: absolute; top: 4px; left: 4px; width: 16px; height: 16px; background: #d7c9c8; border-radius: 50%; box-shadow: 0 1px 3px rgba(0,0,0,.35); transition: transform .18s ease, background .18s ease; }
  .toggle-switch input:checked + .switch-track { background: #ed806f; border-color: #ffad9d; }
  .toggle-switch input:checked + .switch-track::before { background: #fff7f5; transform: translateX(18px); }
  .toggle-switch:hover .switch-track { border-color: #ffad9d; }
  .toggle-switch input:focus-visible + .switch-track { outline: 2px solid #ffc0b4; outline-offset: 3px; }
  .cards { display: grid; grid-template-columns: repeat(2, minmax(0,1fr)); gap: 14px; padding: 18px; margin-top: 14px; background: #281f20; border: 1px solid rgba(255,255,255,.07); border-radius: 14px; }
  .card { min-width: 0; padding: 7px; color: #d8cccb; text-align: center; background: transparent; border: 1px solid transparent; border-radius: 13px; cursor: pointer; transition: background .16s ease, border-color .16s ease, transform .16s ease; }
  .card:hover { background: rgba(255,255,255,.035); transform: translateY(-2px); }
  .card.selected { background: rgba(255,111,85,.07); border-color: rgba(255,154,137,.42); }
  .card:focus-visible, .close:focus-visible { outline: 2px solid #ff9a89; outline-offset: 2px; }
  .nav:focus-visible { outline: 2px solid #ff9a89; outline-offset: 2px; z-index: 1; }
  .preview { position: relative; display: block; width: 100%; height: clamp(100px, 20vh, 156px); overflow: hidden; border: 1px solid rgba(255,255,255,.14); border-radius: 10px; background: #151112; }
  .preview img { width: 100%; height: 100%; object-fit: cover; }
  #choose-background:not(.selected) .preview img { display: none; }
  #choose-background:not(.selected) .preview::after { content: "+\A Choose an image"; white-space: pre; position: absolute; inset: 0; display: grid; align-content: center; justify-items: center; gap: 8px; color: #e6d4d1; font-size: 14px; background: linear-gradient(145deg,#2a2021,#1c1617); }
  #solid-background .preview { background: linear-gradient(145deg,#172744,#243e66); }
  #gradient-background .preview { background: linear-gradient(135deg,#ed765e 0%,#a64fa1 48%,#394f9d 100%); }
  .preview .select-mark { position: absolute; right: 12px; bottom: 12px; display: none; place-items: center; width: 30px; height: 30px; color: transparent; font-size: 0; background: #ffaaa0; border: 3px solid #fff; border-radius: 50%; box-shadow: 0 3px 10px rgba(0,0,0,.38); }
  .preview .select-mark::after { content: ""; width: 7px; height: 12px; margin-top: -3px; border: solid #fff; border-width: 0 3px 3px 0; transform: rotate(45deg); }
  .card.selected .select-mark { display: grid; }
  .card-label { display: block; padding: 8px 0 2px; font-size: 13px; font-weight: 550; }
  .card:hover .preview, .card.selected .preview { border-color: rgba(255,154,137,.58); }
  .engine-heading { margin: 0 8px 12px; color: #d7c8c6; font-size: 13px; font-weight: 600; }
  .engine-list { overflow: hidden; background: #211a1a; border: 1px solid rgba(255,255,255,.1); border-radius: 10px; }
  .engine-option { display: flex; align-items: center; gap: 14px; min-height: 52px; padding: 8px 16px; color: #e7dcda; cursor: pointer; transition: background .15s ease; }
  .engine-option + .engine-option { border-top: 1px solid rgba(255,255,255,.09); }
  .engine-option:hover { background: rgba(255,255,255,.045); }
  .engine-option input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .engine-option:focus-within { outline: 2px solid #ff9a89; outline-offset: -2px; z-index: 1; }
  .engine-icon { display: block; flex: 0 0 24px; width: 24px; height: 24px; object-fit: contain; }
  .engine-name { flex: 1; color: #e2d7d5; font-size: 14px; font-weight: 450; }
  .engine-check { display: grid; flex: 0 0 16px; width: 16px; height: 16px; place-items: center; border: 1.5px solid #aa9997; border-radius: 4px; transition: background .15s ease, border-color .15s ease; }
  .engine-check::after { content: ""; width: 4px; height: 8px; margin-top: -2px; border: solid transparent; border-width: 0 2px 2px 0; transform: rotate(45deg); }
  .engine-option.selected .engine-check { background: #ffab9d; border-color: #ffab9d; }
  .engine-option.selected .engine-check::after { border-color: #372321; }
  .hint { margin: 14px 4px; color: #b7aaaa; font-size: 13px; line-height: 1.5; }
  #wallpaper-file { display: none; }
  @media (max-width: 700px) {
    .customize { inset: 0; width: 100vw; height: 100vh; transform: none; border: 0; border-radius: 0; }
    .side { display: none; }
    .content { padding: 38px 32px 24px; }
    .panel { max-width: none; }
    .preview { height: clamp(140px, 30vh, 180px); }
  }
  @media (max-width: 420px) {
    .content { padding-inline: 20px; }
    .cards { gap: 8px; padding: 10px; }
    .preview { height: clamp(110px, 28vh, 150px); }
    .card-label { font-size: 12px; }
  }
  @media (max-height: 480px) { .customize { height: calc(100vh - 16px); } .content { padding-top: 24px; } }
</style></head>
<body class="{{BACKGROUND_CLASS}}" style='--wallpaper: url("{{WALLPAPER_URI}}")'>
<button id="customize-button" class="gear" aria-label="Customize new tab" title="Customize">
  <img alt="" src="data:image/png;base64,{{SETTINGS_ICON}}">
</button>
<main class="stage">
  <form id="f">
    <img alt="" src="data:image/png;base64,{{LOGO}}">
    <input id="q" placeholder="Search or enter an address..." autofocus autocomplete="off">
  </form>
  <div class="mode">{{HEADING}}</div>
  <div class="tiles">{{TILES}}</div>
</main>

<div class="customize-backdrop" id="customize-backdrop"></div>
<section class="customize" id="customize" role="dialog" aria-modal="true" aria-label="Customize new tab">
  <nav class="side">
    <h1>Customize</h1>
    <button class="nav active" data-panel="background-panel">▧ <span>Background Image</span></button>
    <button class="nav" data-panel="search-panel">⌕ <span>Search</span></button>
  </nav>
  <main class="content">
    <button class="close" id="close-customize" aria-label="Close">×</button>
    <section class="panel active" id="background-panel">
      <h2>Background Image</h2>
      <div class="toggle-row"><span>Show background images</span><label class="toggle-switch" aria-label="Show background images"><input id="show-background" type="checkbox" {{BACKGROUND_CHECKED}}><span class="switch-track" aria-hidden="true"></span></label></div>
      <div class="cards">
        <button class="card {{DEFAULT_SELECTED}}" id="default-background">
          <span class="preview"><img alt="Linus Browser background" src="data:image/png;base64,{{DEFAULT_WALLPAPER}}"><span class="select-mark">✓</span></span>
          <span class="card-label">Linus background</span>
        </button>
        <button class="card {{CUSTOM_SELECTED}}" id="choose-background">
          <span class="preview"><img id="custom-preview" alt="Choose your own background" src="{{WALLPAPER_URI}}"><span class="select-mark">✓</span></span>
          <span class="card-label">Use your own</span>
        </button>
        <button class="card wallpaper-card {{SOLID_SELECTED}}" id="solid-background" data-style="solid">
          <span class="preview"><span class="select-mark">✓</span></span>
          <span class="card-label">Solid color</span>
        </button>
        <button class="card wallpaper-card {{GRADIENT_SELECTED}}" id="gradient-background" data-style="gradient">
          <span class="preview"><span class="select-mark">✓</span></span>
          <span class="card-label">Gradient</span>
        </button>
      </div>
      <input id="wallpaper-file" type="file" accept="image/png,image/jpeg,image/webp">
    </section>
    <section class="panel" id="search-panel">
      <h2>Search</h2>
      <p class="engine-heading">Default search engine</p>
      <div class="engine-list" role="radiogroup" aria-label="Default search engine">
        <label class="engine-option {{DDG_SELECTED}}">
          <input type="radio" name="search-engine" value="duckduckgo" {{DDG_SELECTED}} aria-label="DuckDuckGo">
          <img class="engine-icon" alt="" src="{{DUCKDUCKGO_LOGO}}">
          <span class="engine-name">DuckDuckGo</span>
          <span class="engine-check" aria-hidden="true"></span>
        </label>
        <label class="engine-option {{GOOGLE_SELECTED}}">
          <input type="radio" name="search-engine" value="google" {{GOOGLE_SELECTED}} aria-label="Google">
          <img class="engine-icon" alt="" src="{{GOOGLE_LOGO}}">
          <span class="engine-name">Google</span>
          <span class="engine-check" aria-hidden="true"></span>
        </label>
        <label class="engine-option {{BRAVE_SELECTED}}">
          <input type="radio" name="search-engine" value="brave" {{BRAVE_SELECTED}} aria-label="Brave Search">
          <img class="engine-icon" alt="" src="{{BRAVE_LOGO}}">
          <span class="engine-name">Brave Search</span>
          <span class="engine-check" aria-hidden="true"></span>
        </label>
      </div>
      <p class="hint">Your choice is used for searches from the new-tab page and address bar.</p>
    </section>
  </main>
</section>
<script>
  var customize = document.getElementById('customize');
  var customizeBackdrop = document.getElementById('customize-backdrop');
  var defaultWallpaper = 'data:image/png;base64,{{DEFAULT_WALLPAPER}}';
  var searchUrls = {
    duckduckgo: 'https://duckduckgo.com/?q=',
    google: 'https://www.google.com/search?q=',
    brave: 'https://search.brave.com/search?q='
  };
  function send(message) {
    if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.linus)
      window.webkit.messageHandlers.linus.postMessage(message);
  }
  function closeCustomize() {
    customize.classList.remove('open');
    customizeBackdrop.classList.remove('open');
  }
  function setWallpaperStyle(style) {
    document.body.classList.remove('wallpaper-solid', 'wallpaper-gradient');
    if (style === 'solid' || style === 'gradient')
      document.body.classList.add('wallpaper-' + style);
    document.querySelectorAll('.wallpaper-card, #default-background, #choose-background').forEach(function (card) {
      card.classList.toggle('selected', card.dataset.style === style || card.id === (style === 'default' ? 'default-background' : style === 'custom' ? 'choose-background' : ''));
    });
    document.body.classList.remove('no-background');
    document.getElementById('show-background').checked = true;
    send('wallpaper-style:' + style);
    send('background:1');
  }
  document.getElementById('customize-button').addEventListener('click', function () {
    customize.classList.add('open');
    customizeBackdrop.classList.add('open');
  });
  document.getElementById('close-customize').addEventListener('click', function () {
    closeCustomize();
  });
  customizeBackdrop.addEventListener('click', function () {
    closeCustomize();
  });
  document.addEventListener('keydown', function (event) {
    if (event.key === 'Escape') closeCustomize();
  });
  document.querySelectorAll('.nav').forEach(function (button) {
    button.addEventListener('click', function () {
      document.querySelectorAll('.nav').forEach(function (item) { item.classList.remove('active'); });
      document.querySelectorAll('.panel').forEach(function (item) { item.classList.remove('active'); });
      button.classList.add('active');
      document.getElementById(button.dataset.panel).classList.add('active');
    });
  });
  document.getElementById('show-background').addEventListener('change', function (event) {
    document.body.classList.toggle('no-background', !event.target.checked);
    send('background:' + (event.target.checked ? '1' : '0'));
  });
  document.getElementById('default-background').addEventListener('click', function () {
    document.body.style.setProperty('--wallpaper', 'url("' + defaultWallpaper + '")');
    setWallpaperStyle('default');
    document.getElementById('custom-preview').src = defaultWallpaper;
    send('wallpaper:default');
  });
  document.getElementById('choose-background').addEventListener('click', function () {
    document.getElementById('wallpaper-file').click();
  });
  document.getElementById('wallpaper-file').addEventListener('change', function (event) {
    var file = event.target.files && event.target.files[0];
    if (!file) return;
    if (file.size > 10485760) { alert('Choose an image smaller than 10 MB.'); return; }
    var reader = new FileReader();
    reader.onload = function () {
      var image = String(reader.result);
      document.body.style.setProperty('--wallpaper', 'url("' + image + '")');
      document.getElementById('custom-preview').src = image;
      setWallpaperStyle('custom');
      send('wallpaper:' + image);
    };
    reader.readAsDataURL(file);
  });
  document.querySelectorAll('.wallpaper-card').forEach(function (card) {
    card.addEventListener('click', function () {
      setWallpaperStyle(card.dataset.style);
    });
  });
  document.querySelectorAll('input[name="search-engine"]').forEach(function (input) {
    input.addEventListener('change', function () {
      document.querySelectorAll('.engine-option').forEach(function (option) {
        option.classList.toggle('selected', option.querySelector('input').checked);
      });
      send('search:' + input.value);
    });
  });
  document.getElementById('f').addEventListener('submit', function (event) {
    event.preventDefault();
    var value = document.getElementById('q').value.trim();
    if (!value) return;
    var looksLikeUrl = /^[^\s]+\.[^\s]+$/.test(value);
    location.href = looksLikeUrl
      ? (value.indexOf('://') > 0 ? value : 'https://' + value)
      : searchUrls[document.querySelector('input[name="search-engine"]:checked').value] + encodeURIComponent(value);
  });
</script>
</body></html>"##;

const INTERNAL_PAGE: &str = r##"<!doctype html>
<html><head><meta charset="utf-8"><title>{{TITLE}}</title>
<style>
  body { margin: 0; padding: 32px 48px; background: #17171a; color: #eee; font-family: system-ui, sans-serif; }
  h1 { font-weight: 400; border-bottom: 1px solid #3a2a28; padding-bottom: 12px; }
  ul { list-style: none; padding: 0; max-width: 900px; }
  li { display: flex; gap: 16px; align-items: baseline; padding: 8px 10px; border-radius: 8px; }
  li:hover { background: #24242a; }
  .t { color: #9a8f8f; font-size: 12px; min-width: 120px; }
  a { color: #ff9a85; text-decoration: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  a:hover { text-decoration: underline; }
  small { color: #7d7676; }
  .empty { color: #8b8181; }
</style></head><body><h1>{{TITLE}}</h1>{{BODY}}</body></html>"##;

const HISTORY_PAGE: &str = r##"<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>History</title>
<style>
  * { box-sizing: border-box; }
  body { display: grid; grid-template-columns: 256px minmax(0,1fr); margin: 0; min-height: 100vh; background: #202123; color: #eee9e8; font: 14px system-ui, sans-serif; }
  .history-sidebar { position: sticky; top: 0; display: flex; height: 100vh; flex-direction: column; padding: 18px 12px; background: #202123; border-right: 1px solid rgba(255,255,255,.06); }
  .history-brand { display: flex; align-items: center; gap: 12px; padding: 0 10px 24px; color: #e9e4e3; font-size: 21px; font-weight: 600; }
  .history-brand-icon { display: grid; width: 32px; height: 32px; place-items: center; color: #ff9a89; background: #302425; border: 1px solid #493435; border-radius: 9px; font-size: 20px; }
  .history-nav { display: grid; gap: 4px; }
  .history-nav button { display: flex; align-items: center; gap: 12px; min-height: 40px; padding: 0 12px; color: #c9c4c3; text-align: left; background: transparent; border: 0; border-radius: 8px; font: inherit; cursor: pointer; }
  .history-nav button:hover { color: #fff; background: rgba(255,255,255,.06); }
  .history-nav button.current { color: #ffaaa0; background: rgba(255,111,85,.11); }
  .history-nav button:focus-visible { outline: 2px solid #ff9a89; outline-offset: -2px; }
  .history-nav-icon { width: 18px; color: #b7aaaa; text-align: center; }
  .history-page { width: min(100%, 1440px); min-width: 0; margin: 0 auto; padding: 8px clamp(28px, 4vw, 64px) 60px; }
  .history-header { display: flex; align-items: center; justify-content: center; padding: 0 0 16px; }
  .heading { display: none; }
  .heading h1 { margin: 0; font-size: 23px; font-weight: 600; letter-spacing: -.015em; }
  .heading p { display: none; }
  .history-count { display: none; }
  .history-search { width: min(690px, 100%); height: 44px; padding: 0 14px 0 40px; color: #f2e9e7; background: #1b1c1e; border: 1px solid #3c3d40; border-radius: 10px; outline: none; }
  .history-search::placeholder { color: #a39b99; }
  .history-search:focus { border-color: #b6796f; box-shadow: 0 0 0 2px rgba(232,137,122,.12); }
  .history-actions { position: relative; width: min(690px, 100%); }
  .history-actions::before { content: "⌕"; position: absolute; top: 5px; left: 14px; color: #b8acab; font-size: 24px; line-height: 30px; }
  .history-nav .clear-history { position: static; width: 100%; height: auto; min-height: 40px; overflow: visible; clip-path: none; padding: 0 12px; color: #c9c4c3; white-space: normal; }
  .history-nav .clear-history:hover { color: #ffaaa0; background: rgba(235,111,95,.1); }
  .day-group { margin: 14px 0 16px; padding: 18px 20px 10px; background: #292a2d; border: 1px solid rgba(0,0,0,.24); border-radius: 10px; box-shadow: 0 2px 6px rgba(0,0,0,.16); }
  .day-group[hidden], .visit[hidden] { display: none; }
  .day-heading { margin: 0 0 12px 2px; color: #e5dddd; font-size: 15px; font-weight: 550; }
  .visit-list { display: grid; }
  .visit { display: flex; min-height: 43px; align-items: center; gap: 14px; padding: 5px 4px; color: inherit; text-decoration: none; border-radius: 6px; transition: background .14s; }
  .visit:hover { background: rgba(255,255,255,.055); }
  .visit time { order: 1; flex: 0 0 94px; color: #c1b8b7; font-size: 12px; font-variant-numeric: tabular-nums; }
  .site-icon { order: 2; position: relative; display: grid; flex: 0 0 18px; width: 18px; height: 20px; place-items: center; overflow: hidden; color: #42a16a; font-size: 14px; font-weight: 700; }
  .site-icon img { position: absolute; inset: 0; display: block; width: 18px; height: 18px; object-fit: contain; }
  .site-icon img[hidden] { display: none; }
  .visit-info { order: 3; display: flex; flex: 1; min-width: 0; align-items: baseline; gap: 8px; }
  .visit-title { overflow: hidden; color: #e8e2e0; font-size: 13px; font-weight: 450; text-overflow: ellipsis; white-space: nowrap; }
  .visit-url { overflow: hidden; color: #b5abaa; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .empty-state { display: grid; justify-items: center; padding: 100px 20px; color: #a99d9b; text-align: center; }
  .empty-icon { display: grid; width: 54px; height: 54px; place-items: center; color: #e59b8d; background: #2a2021; border: 1px solid #493435; border-radius: 17px; font-size: 28px; }
  .empty-state h2 { margin: 18px 0 6px; color: #eee7e5; font-size: 17px; }
  .empty-state p { margin: 0; font-size: 13px; }
  .search-empty { display: none; padding: 44px 20px; color: #a99d9b; text-align: center; }
  @media (max-width: 760px) {
    body { grid-template-columns: 1fr; }
    .history-sidebar { display: none; }
    .history-page { padding: 18px 16px 40px; }
    .history-header { align-items: stretch; flex-direction: column; gap: 14px; }
    .heading { display: block; }
    .history-actions { width: 100%; }
    .history-search { width: 100%; }
    .visit { gap: 9px; }
    .visit time { flex-basis: 74px; }
    .visit-info { display: grid; gap: 2px; }
  }
</style></head>
<body>
<aside class="history-sidebar">
  <div class="history-brand"><span class="history-brand-icon">◷</span>History</div>
  <nav class="history-nav" aria-label="History navigation">
    <button class="current" type="button"><span class="history-nav-icon">◷</span>History</button>
    {{CLEAR_BUTTON}}
  </nav>
</aside>
<main class="history-page">
  <header class="history-header">
    <div class="heading"><h1>History <span class="history-count" id="history-count">{{HISTORY_COUNT}} visits</span></h1><p>Your recent browsing activity</p></div>
    <div class="history-actions">
      <input class="history-search" id="history-search" type="search" placeholder="Search history" aria-label="Search history" autocomplete="off">
    </div>
  </header>
  <div id="history-content">{{HISTORY_CONTENT}}</div>
  <div class="search-empty" id="search-empty">No matching pages found.</div>
</main>
<script>
  var search = document.getElementById('history-search');
  search.addEventListener('input', function () {
    var query = search.value.trim().toLowerCase();
    var visible = 0;
    document.querySelectorAll('.day-group').forEach(function (group) {
      var groupVisible = 0;
      group.querySelectorAll('.visit').forEach(function (visit) {
        var matches = !query || visit.dataset.search.indexOf(query) !== -1;
        visit.hidden = !matches;
        if (matches) groupVisible++;
      });
      group.hidden = groupVisible === 0;
      visible += groupVisible;
    });
    document.getElementById('search-empty').style.display = query && visible === 0 ? 'block' : 'none';
    document.getElementById('history-count').textContent = visible + (visible === 1 ? ' visit' : ' visits');
  });
  var clearButton = document.getElementById('clear-history');
  if (clearButton) clearButton.addEventListener('click', function () {
    if (window.confirm('Clear all browsing history?'))
      location.href = 'tux://history/clear';
  });
</script></body></html>"##;

fn theme_css_path() -> PathBuf {
    glib::user_config_dir()
        .join("linus_browser")
        .join("theme.css")
}

fn wallpaper_palette_path() -> PathBuf {
    glib::user_config_dir()
        .join("linus_browser")
        .join("wallpaper-palette.css")
}

fn wallpaper_sources() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("LINUS_BROWSER_WALLPAPER") {
        paths.push(PathBuf::from(path));
    }
    paths.push(glib::home_dir().join(".local/state/omarchy/current/background"));
    paths.push(glib::user_config_dir().join("hypr/wallpaper.png"));
    paths
}

fn current_wallpaper() -> Option<PathBuf> {
    wallpaper_sources()
        .into_iter()
        .find(|path| path.is_file())
        .and_then(|path| fs::canonicalize(path).ok())
}

fn palette_css_from_bgra(
    width: usize,
    height: usize,
    stride: usize,
    pixels: &[u8],
) -> Result<String, String> {
    if width == 0 || height == 0 || stride < width.saturating_mul(4) {
        return Err("invalid wallpaper pixel dimensions".to_string());
    }
    let required = stride
        .checked_mul(height)
        .ok_or_else(|| "wallpaper pixel buffer is too large".to_string())?;
    if pixels.len() < required || width.saturating_mul(height) > 20_000_000 {
        return Err("wallpaper pixel buffer is incomplete or too large".to_string());
    }

    let step_x = (width / 64).max(1);
    let step_y = (height / 64).max(1);
    type ColorBin = (u32, u64, u64, u64);
    let mut bins: HashMap<(u8, u8, u8), ColorBin> = HashMap::new();
    for y in (0..height).step_by(step_y) {
        for x in (0..width).step_by(step_x) {
            let offset = y * stride + x * 4;
            #[cfg(target_endian = "little")]
            let (blue, green, red, alpha) = (
                pixels[offset],
                pixels[offset + 1],
                pixels[offset + 2],
                pixels[offset + 3],
            );
            #[cfg(target_endian = "big")]
            let (alpha, red, green, blue) = (
                pixels[offset],
                pixels[offset + 1],
                pixels[offset + 2],
                pixels[offset + 3],
            );
            if alpha < 96 {
                continue;
            }
            let key = (red >> 4, green >> 4, blue >> 4);
            let bin = bins.entry(key).or_default();
            bin.0 += 1;
            bin.1 += u64::from(red);
            bin.2 += u64::from(green);
            bin.3 += u64::from(blue);
        }
    }
    if bins.is_empty() {
        return Err("wallpaper has no visible pixels".to_string());
    }

    let average = |bin: &(u32, u64, u64, u64)| {
        (
            (bin.1 / u64::from(bin.0)) as u8,
            (bin.2 / u64::from(bin.0)) as u8,
            (bin.3 / u64::from(bin.0)) as u8,
        )
    };
    let dominant = bins
        .values()
        .max_by_key(|bin| bin.0)
        .map(average)
        .expect("non-empty color bins have a dominant color");
    let accent = bins
        .values()
        .filter_map(|bin| {
            let color = average(bin);
            let max = f32::from(color.0.max(color.1).max(color.2)) / 255.0;
            let min = f32::from(color.0.min(color.1).min(color.2)) / 255.0;
            let saturation = if max == 0.0 { 0.0 } else { (max - min) / max };
            (saturation >= 0.28 && (0.2..=0.95).contains(&max))
                .then_some((bin.0 as f32 * (0.25 + saturation * saturation), color))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, color)| color)
        .unwrap_or(dominant);

    let (r, g, b) = dominant;
    let (ar, ag, ab) = accent;

    // The wallpaper contributes a faint hue tint to the dark ramp and, when it
    // is vivid enough, the accent. It never sets lightness: that is what made
    // the old theme collapse into a single flat grey slab on dark wallpapers.
    let tint = 0.10;
    let tinted = |base: f32| -> String {
        let mix = |channel: u8| {
            (base * (1.0 - tint) + f32::from(channel) * tint).round() as u8
        };
        format!("rgb({}, {}, {})", mix(r), mix(g), mix(b))
    };
    // Only trust the wallpaper for the accent when it yields a genuinely vivid
    // color. Dark/muted wallpapers otherwise sample near-black (e.g. #370000),
    // which makes focus rings and progress bars invisible.
    let accent_saturation = {
        let max = f32::from(ar.max(ag).max(ab));
        let min = f32::from(ar.min(ag).min(ab));
        if max == 0.0 { 0.0 } else { (max - min) / max }
    };
    let accent_brightness = f32::from(ar.max(ag).max(ab)) / 255.0;
    let (ar, ag, ab) = if accent_saturation < 0.55 || accent_brightness < 0.45 {
        (255, 106, 77) // design-token accent
    } else {
        (ar, ag, ab)
    };
    let accent_hex = format!("#{ar:02x}{ag:02x}{ab:02x}");

    Ok(format!(
        "@define-color shell {};\n\
         @define-color chrome {};\n\
         @define-color surface {};\n\
         @define-color surface_hi {};\n\
         @define-color window_bg @shell;\n\
         @define-color window_bg_color @shell;\n\
         @define-color accent_color {accent_hex};\n\
         @define-color accent_rgb rgb({ar}, {ag}, {ab});\n\
         @define-color focus_ring rgba({ar}, {ag}, {ab}, 0.35);\n",
        tinted(22.0),
        tinted(32.0),
        tinted(42.0),
        tinted(52.0),
    ))
}

fn wallpaper_palette_css() -> Option<String> {
    let path = current_wallpaper()?;
    let texture = match gdk::Texture::from_filename(&path) {
        Ok(texture) => texture,
        Err(error) => {
            eprintln!("Could not load wallpaper {}: {error}", path.display());
            return None;
        }
    };
    let width = texture.width() as usize;
    let height = texture.height() as usize;
    if width == 0 || height == 0 || width.saturating_mul(height) > 20_000_000 {
        eprintln!(
            "Ignoring unsupported wallpaper dimensions: {}",
            path.display()
        );
        return None;
    }
    let stride = width.saturating_mul(4);
    let mut pixels = vec![0; stride * height];
    texture.download(&mut pixels, stride);
    match palette_css_from_bgra(width, height, stride, &pixels) {
        Ok(css) => {
            if let Err(error) = fs::write(wallpaper_palette_path(), &css) {
                eprintln!("Could not write wallpaper palette CSS: {error}");
            }
            Some(css)
        }
        Err(error) => {
            eprintln!("Could not extract wallpaper palette: {error}");
            None
        }
    }
}

fn composed_theme_css(palette_css: &str, user_css: &str) -> String {
    format!("{CSS}\n{palette_css}\n{user_css}")
}

fn css_is_valid(css: &str) -> bool {
    let provider = CssProvider::new();
    let valid = Rc::new(Cell::new(true));
    let valid_on_error = valid.clone();
    provider.connect_parsing_error(move |_, _, error| {
        eprintln!("Ignoring invalid dynamic GTK CSS: {error}");
        valid_on_error.set(false);
    });
    provider.load_from_string(css);
    valid.get()
}

fn load_css() -> Vec<gio::FileMonitor> {
    let theme_path = theme_css_path();
    let theme_dir = theme_path
        .parent()
        .expect("theme.css path has a parent directory");
    if let Err(error) = fs::create_dir_all(theme_dir) {
        eprintln!("Could not create browser theme directory: {error}");
    }

    let provider = CssProvider::new();
    if let Some(display) = gdk::Display::default() {
        style_context_add_provider_for_display(&display, &provider, STYLE_PROVIDER_PRIORITY_USER);
    }

    let initial_palette = wallpaper_palette_css()
        .unwrap_or_else(|| fs::read_to_string(wallpaper_palette_path()).unwrap_or_default());
    let generated_css = Rc::new(RefCell::new(initial_palette));
    let reload = {
        let provider = provider.clone();
        let generated_css = generated_css.clone();
        let theme_path = theme_path.clone();
        Rc::new(move || {
            let user_css = match fs::read_to_string(&theme_path) {
                Ok(css) => css,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(error) => {
                    eprintln!(
                        "Could not read dynamic theme {}: {error}",
                        theme_path.display()
                    );
                    return;
                }
            };
            let css = composed_theme_css(&generated_css.borrow(), &user_css);
            if css_is_valid(&css) {
                provider.load_from_string(&css);
            }
        })
    };
    reload();

    let mut wallpaper_watch_sources = wallpaper_sources();
    if let Some(path) = current_wallpaper() {
        wallpaper_watch_sources.push(path);
    }

    let mut watch_dirs = vec![theme_dir.to_path_buf()];
    for source in &wallpaper_watch_sources {
        if let Some(parent) = source.parent()
            && parent.is_dir()
            && !watch_dirs.iter().any(|dir| dir == parent)
        {
            watch_dirs.push(parent.to_path_buf());
        }
    }

    let mut monitors = Vec::new();
    for dir in watch_dirs {
        let is_theme_dir = dir == theme_dir;
        let watched_names: Vec<_> = if is_theme_dir {
            vec![std::ffi::OsString::from("theme.css")]
        } else {
            wallpaper_watch_sources
                .iter()
                .filter(|source| source.parent().is_some_and(|parent| parent == dir))
                .filter_map(|source| source.file_name().map(std::ffi::OsStr::to_os_string))
                .collect()
        };
        let file = gio::File::for_path(&dir);
        match file.monitor_directory(
            gio::FileMonitorFlags::WATCH_MOVES,
            None::<&gio::Cancellable>,
        ) {
            Ok(monitor) => {
                let reload = reload.clone();
                let generated_css = generated_css.clone();
                let dir = dir.clone();
                monitor.connect_changed(move |_, changed, other, _| {
                    let is_relevant = |file: &gio::File| {
                        file.path().is_some_and(|path| {
                            path.parent() == Some(dir.as_path())
                                && path.file_name().is_some_and(|name| {
                                    watched_names.iter().any(|watched| watched == name)
                                })
                        })
                    };
                    if !is_relevant(changed) && !other.is_some_and(is_relevant) {
                        return;
                    }
                    if !is_theme_dir {
                        if let Some(css) = wallpaper_palette_css() {
                            *generated_css.borrow_mut() = css;
                        } else if current_wallpaper().is_none() {
                            generated_css.borrow_mut().clear();
                        }
                    }
                    reload();
                });
                monitors.push(monitor);
            }
            Err(error) => {
                eprintln!(
                    "Could not monitor theme directory {}: {error}",
                    dir.display()
                );
            }
        }
    }
    monitors
}

// ───────────────────────────── browser state ─────────────────────────────

struct Tab {
    web_view: WebView,
    widget: GtkBox,
    label: Label,
    ucm: UserContentManager,
    filter_on: Rc<Cell<bool>>,
    fingerprint_script: Option<UserScript>,
}

struct Browser {
    window: ApplicationWindow,
    app: Application,
    private: bool,
    session: Option<NetworkSession>, // Some(..) only for private windows
    stack: Stack,
    tabs_box: GtkBox,
    tabs: RefCell<Vec<Tab>>,
    url_entry: Entry,
    back: Button,
    forward: Button,
    reload: Button,
    bookmark: Button,
    progress: ProgressBar,
    bookmarks_bar: GtkBox,
    find_bar: GtkBox,
    find_entry: SearchEntry,
    shields_pop: Popover,
    menu_btn: MenuButton,
    shields_sw: Switch,
    shields_host: Label,
    fingerprint_sw: Switch,
    fingerprint_status: Label,
    fingerprint_seed: Option<String>,
    syncing: Cell<bool>,
    downloads_btn: MenuButton,
    downloads_list: GtkBox,
    logo: gdk::Texture,
    logo_b64: String,
    filter: RefCell<Option<UserContentFilter>>,
    prefs: RefCell<Prefs>,
    bookmarks: RefCell<Vec<Bookmark>>,
    history: RefCell<Vec<HistoryItem>>,
    _theme_monitors: Vec<gio::FileMonitor>,
}

impl Browser {
    fn net_session(&self) -> Option<NetworkSession> {
        self.session.clone().or_else(NetworkSession::default)
    }

    fn current_webview(&self) -> Option<WebView> {
        self.stack
            .visible_child()
            .and_then(|w| w.downcast::<WebView>().ok())
    }

    fn is_current(&self, wv: &WebView) -> bool {
        self.current_webview().as_ref() == Some(wv)
    }

    fn select_tab(&self, wv: &WebView) {
        if !self.tabs.borrow().iter().any(|t| &t.web_view == wv) {
            return;
        }
        self.stack.set_visible_child(wv);
        for t in self.tabs.borrow().iter() {
            if &t.web_view == wv {
                t.widget.add_css_class("active");
            } else {
                t.widget.remove_css_class("active");
            }
        }
        self.sync_ui_for(wv);
        self.sync_url_for(wv);
    }

    fn cycle_tab(&self, delta: isize) {
        let target = {
            let tabs = self.tabs.borrow();
            let n = tabs.len() as isize;
            if n == 0 {
                return;
            }
            let cur = self
                .current_webview()
                .and_then(|c| tabs.iter().position(|t| t.web_view == c))
                .unwrap_or(0) as isize;
            tabs[(cur + delta).rem_euclid(n) as usize].web_view.clone()
        };
        self.select_tab(&target);
    }

    fn close_tab(&self, wv: &WebView) {
        let (idx, widget) = {
            let tabs = self.tabs.borrow();
            match tabs.iter().position(|t| &t.web_view == wv) {
                Some(i) => (i, tabs[i].widget.clone()),
                None => return,
            }
        };
        let was_current = self.is_current(wv);
        self.tabs_box.remove(&widget);
        self.stack.remove(wv);
        self.tabs.borrow_mut().remove(idx);

        let next = {
            let tabs = self.tabs.borrow();
            if tabs.is_empty() {
                None
            } else {
                Some(tabs[idx.min(tabs.len() - 1)].web_view.clone())
            }
        };
        match next {
            None => self.window.close(),
            Some(n) if was_current => self.select_tab(&n),
            Some(_) => {}
        }
    }

    fn sync_ui_for(&self, wv: &WebView) {
        self.back.set_sensitive(wv.can_go_back());
        self.forward.set_sensitive(wv.can_go_forward());

        let loading = wv.is_loading();
        self.reload.set_icon_name(if loading {
            "process-stop-symbolic"
        } else {
            "view-refresh-symbolic"
        });
        self.reload.set_tooltip_text(Some(if loading {
            "Stop (Esc)"
        } else {
            "Reload (Ctrl+R)"
        }));
        self.progress.set_visible(loading);
        self.progress.set_fraction(wv.estimated_load_progress());

        let title = wv.title().map(|t| t.to_string()).unwrap_or_default();
        self.set_window_title(&title);
    }

    fn sync_url_for(&self, wv: &WebView) {
        let uri = wv.uri().map(|u| u.to_string()).unwrap_or_default();
        let shown: &str = if uri.starts_with("about:") {
            ""
        } else {
            uri.as_str()
        };
        self.url_entry.set_text(shown);
        self.update_bookmark_icon(shown);
        self.sync_shields_ui(shown);
    }

    fn sync_shields_ui(&self, uri: &str) {
        let host = host_of(uri);
        let fingerprint_farbling =
            self.prefs.borrow().fingerprint_farbling && self.fingerprint_seed.is_some();
        self.syncing.set(true);
        self.fingerprint_sw.set_active(fingerprint_farbling);
        self.fingerprint_sw
            .set_sensitive(self.fingerprint_seed.is_some());
        self.fingerprint_status.set_text(
            match (self.fingerprint_seed.is_some(), fingerprint_farbling) {
                (false, _) => FINGERPRINT_STATUS_UNAVAILABLE,
                (true, true) => FINGERPRINT_STATUS_ACTIVE,
                (true, false) => FINGERPRINT_STATUS_INACTIVE,
            },
        );
        match &host {
            Some(h) => {
                let off = self.prefs.borrow().shields_off.contains(h);
                self.shields_host.set_text(h);
                self.shields_sw.set_active(!off);
                self.shields_sw.set_sensitive(true);
            }
            None => {
                self.shields_host.set_text("No site loaded");
                self.shields_sw.set_sensitive(false);
            }
        }
        self.syncing.set(false);
    }

    /// Add or remove the tracker filter on one tab to match the site's Shields state.
    fn apply_shields_for(&self, wv: &WebView, host: &str) {
        let want = !self.prefs.borrow().shields_off.contains(host);
        let tabs = self.tabs.borrow();
        let Some(tab) = tabs.iter().find(|t| &t.web_view == wv) else {
            return;
        };
        if tab.filter_on.get() == want {
            return;
        }
        if let Some(f) = self.filter.borrow().as_ref() {
            if want {
                tab.ucm.add_filter(f);
            } else {
                tab.ucm.remove_filter(f);
            }
            tab.filter_on.set(want);
        }
    }

    fn apply_fingerprint_farbling(&self, enabled: bool) {
        let mut tabs = self.tabs.borrow_mut();
        for tab in tabs.iter_mut() {
            if let Some(script) = tab.fingerprint_script.take() {
                tab.ucm.remove_script(&script);
            }
            if enabled && let Some(seed) = self.fingerprint_seed.as_deref() {
                let script = fingerprint_farbling_script(seed);
                tab.ucm.add_script(&script);
                tab.fingerprint_script = Some(script);
            }
            if let Some(settings) = WebViewExt::settings(&tab.web_view) {
                settings.set_enable_webgl(true);
                settings.set_enable_webrtc(true);
            }
            tab.web_view.reload();
        }
    }

    fn update_bookmark_icon(&self, uri: &str) {
        let on = self.bookmarks.borrow().iter().any(|b| b.url == uri);
        self.bookmark.set_icon_name(if on {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        });
    }

    fn set_window_title(&self, page_title: &str) {
        let suffix = if self.private {
            format!("{APP_TITLE} (Private)")
        } else {
            APP_TITLE.to_string()
        };
        if page_title.is_empty() {
            self.window.set_title(Some(&suffix));
        } else {
            self.window
                .set_title(Some(&format!("{page_title} — {suffix}")));
        }
    }

    fn navigate(&self, input: &str) {
        let Some(wv) = self.current_webview() else {
            return;
        };
        let t = input.trim();
        if let Some(page) = t.strip_prefix("tux://") {
            if self.show_internal(&wv, page.trim_end_matches('/')) {
                return;
            }
        }
        wv.load_uri(&normalize_input(t, &self.prefs.borrow().search_engine));
    }

    fn reload_current(&self) {
        let Some(wv) = self.current_webview() else {
            return;
        };
        let uri = wv.uri().map(|u| u.to_string()).unwrap_or_default();
        if let Some(page) = uri.strip_prefix("tux://") {
            if self.show_internal(&wv, page) {
                return;
            }
        }
        if uri.is_empty() || uri.starts_with("about:") {
            wv.load_html(&self.new_tab_page(), None);
        } else {
            wv.reload();
        }
    }

    fn record_history(&self, wv: &WebView) {
        if self.private {
            return;
        }
        let uri = wv.uri().map(|u| u.to_string()).unwrap_or_default();
        if !(uri.starts_with("http://") || uri.starts_with("https://")) {
            return;
        }
        save_history_favicon(wv);
        let title = wv.title().map(|t| t.to_string()).unwrap_or_default();
        let time = glib::real_time() / 1_000_000;
        {
            let hist = self.history.borrow();
            if hist.last().map(|h| h.url == uri).unwrap_or(false) {
                return;
            }
        }
        append_line("history.tsv", &format!("{time}\t{}\t{uri}", clean(&title)));
        self.history.borrow_mut().push(HistoryItem {
            time,
            title,
            url: uri,
        });
    }

    fn clear_history(&self) {
        self.history.borrow_mut().clear();
        if !self.private
            && let Err(error) = fs::write(data_dir().join("history.tsv"), "")
        {
            eprintln!("Could not clear browsing history: {error}");
        }
    }

    // ----- generated pages -----

    fn new_tab_page(&self) -> String {
        let prefs = self.prefs.borrow();
        let custom_background = if prefs.wallpaper_style == "custom" {
            fs::read(data_dir().join("new-tab-background")).ok()
        } else {
            None
        };
        let wallpaper_style =
            effective_wallpaper_style(&prefs.wallpaper_style, custom_background.is_some());
        let wallpaper_bytes = custom_background
            .as_deref()
            .unwrap_or(DEFAULT_NEW_TAB_BACKGROUND);
        let wallpaper_mime = if custom_background.is_some() {
            match fs::read_to_string(data_dir().join("new-tab-background-mime"))
                .as_deref()
                .unwrap_or("image/png")
            {
                "image/jpeg" => "image/jpeg",
                "image/webp" => "image/webp",
                _ => "image/png",
            }
        } else {
            "image/png"
        };
        let wallpaper_uri = format!(
            "data:{wallpaper_mime};base64,{}",
            glib::base64_encode(wallpaper_bytes)
        );
        let default_wallpaper = glib::base64_encode(DEFAULT_NEW_TAB_BACKGROUND);
        let duckduckgo_logo = format!(
            "data:image/x-icon;base64,{}",
            glib::base64_encode(DUCKDUCKGO_SEARCH_ICON)
        );
        let google_logo = format!(
            "data:image/png;base64,{}",
            glib::base64_encode(GOOGLE_SEARCH_ICON)
        );
        let brave_logo = format!(
            "data:image/svg+xml;base64,{}",
            glib::base64_encode(BRAVE_SEARCH_ICON)
        );
        let search_engine = prefs.search_engine.as_str();
        let show_background = prefs.background_images;
        let tiles: String = self
            .bookmarks
            .borrow()
            .iter()
            .filter(|b| b.url.starts_with("http"))
            .take(8)
            .map(|b| {
                let initial = b
                    .title
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                format!(
                    r#"<a class="tile" href="{}"><div class="ic">{}</div><span>{}</span></a>"#,
                    esc(&b.url),
                    esc(&initial),
                    esc(&b.title)
                )
            })
            .collect();
        NEW_TAB_HTML
            .replace("{{LOGO}}", &self.logo_b64)
            .replace(
                "{{SETTINGS_ICON}}",
                glib::base64_encode(SETTINGS_GEAR_PNG).as_str(),
            )
            .replace("{{WALLPAPER_URI}}", &wallpaper_uri)
            .replace("{{DEFAULT_WALLPAPER}}", default_wallpaper.as_str())
            .replace("{{DUCKDUCKGO_LOGO}}", &duckduckgo_logo)
            .replace("{{GOOGLE_LOGO}}", &google_logo)
            .replace("{{BRAVE_LOGO}}", &brave_logo)
            .replace(
                "{{BACKGROUND_CLASS}}",
                &format!(
                    "{} wallpaper-{}",
                    if show_background { "" } else { "no-background" },
                    wallpaper_style
                ),
            )
            .replace(
                "{{BACKGROUND_CHECKED}}",
                if show_background { "checked" } else { "" },
            )
            .replace(
                "{{DEFAULT_SELECTED}}",
                if wallpaper_style == "default" {
                    "selected"
                } else {
                    ""
                },
            )
            .replace(
                "{{CUSTOM_SELECTED}}",
                if wallpaper_style == "custom" {
                    "selected"
                } else {
                    ""
                },
            )
            .replace(
                "{{SOLID_SELECTED}}",
                if wallpaper_style == "solid" {
                    "selected"
                } else {
                    ""
                },
            )
            .replace(
                "{{GRADIENT_SELECTED}}",
                if wallpaper_style == "gradient" {
                    "selected"
                } else {
                    ""
                },
            )
            .replace(
                "{{DDG_SELECTED}}",
                if search_engine == "duckduckgo" {
                    "selected"
                } else {
                    ""
                },
            )
            .replace(
                "{{GOOGLE_SELECTED}}",
                if search_engine == "google" {
                    "selected"
                } else {
                    ""
                },
            )
            .replace(
                "{{BRAVE_SELECTED}}",
                if search_engine == "brave" {
                    "selected"
                } else {
                    ""
                },
            )
            .replace(
                "{{GLOW}}",
                if self.private {
                    "140,90,255"
                } else {
                    "255,0,0"
                },
            )
            .replace(
                "{{HEADING}}",
                if self.private {
                    "This is a private window"
                } else {
                    ""
                },
            )
            .replace("{{TILES}}", &tiles)
    }

    fn internal_html(title: &str, body: &str) -> String {
        INTERNAL_PAGE
            .replace("{{TITLE}}", title)
            .replace("{{BODY}}", body)
    }

    fn history_html(&self) -> String {
        let hist = self.history.borrow();
        if hist.is_empty() {
            let empty_message = if self.private {
                "Pages visited in this private window are not saved here."
            } else {
                "Pages you visit will appear here."
            };
            return HISTORY_PAGE
                .replace("{{HISTORY_COUNT}}", "0")
                .replace("{{CLEAR_BUTTON}}", "")
                .replace("{{HISTORY_CONTENT}}", &format!(
                    r#"<div class="empty-state"><span class="empty-icon">◷</span><h2>No history yet</h2><p>{empty_message}</p></div>"#
                ));
        }

        let clear_button = if self.private {
            String::new()
        } else {
            r#"<button class="clear-history" id="clear-history" type="button"><span class="history-nav-icon">⌫</span>Clear browsing data</button>"#.to_string()
        };
        let today = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let today_key = format_history_date(today, "%Y-%m-%d");
        let yesterday_key = format_history_date(today - 86_400, "%Y-%m-%d");
        let mut groups: Vec<(String, String, Vec<&HistoryItem>)> = Vec::new();
        for item in hist.iter().rev().take(500) {
            let date_key = format_history_date(item.time, "%Y-%m-%d");
            let date = format_history_date(item.time, "%A, %B %-d, %Y");
            let date_label = if date_key == today_key {
                format!("Today - {date}")
            } else if date_key == yesterday_key {
                format!("Yesterday - {date}")
            } else {
                date
            };
            if groups.last().is_none_or(|(key, _, _)| key != &date_key) {
                groups.push((date_key.clone(), date_label, Vec::new()));
            }
            if let Some((_, _, entries)) = groups.last_mut() {
                entries.push(item);
            }
        }

        let mut body = String::new();
        for (_, label, entries) in groups {
            body.push_str(&format!(
                r#"<section class="day-group"><h2 class="day-heading">{}</h2><div class="visit-list">"#,
                esc(&label)
            ));
            for item in entries {
                let title = if item.title.trim().is_empty() {
                    item.url.as_str()
                } else {
                    item.title.as_str()
                };
                let host = host_of(&item.url).unwrap_or_default();
                let time = format_history_date(item.time, "%-I:%M %p");
                let search_text = format!("{title} {host} {}", item.url);
                let favicon = history_favicon_src(&item.url);
                body.push_str(&format!(
                    r#"<a class="visit" href="{}" data-search="{}">{}<span class="visit-info"><span class="visit-title">{}</span><span class="visit-url">{}</span></span><time>{}</time></a>"#,
                    esc(&item.url),
                    esc(&search_text.to_lowercase()),
                    history_site_icon_html(&favicon),
                    esc(title),
                    esc(&host),
                    esc(&time)
                ));
            }
            body.push_str("</div></section>");
        }
        HISTORY_PAGE
            .replace("{{HISTORY_COUNT}}", &hist.len().min(500).to_string())
            .replace("{{CLEAR_BUTTON}}", &clear_button)
            .replace("{{HISTORY_CONTENT}}", &body)
    }

    fn bookmarks_html(&self) -> String {
        let list = self.bookmarks.borrow();
        if list.is_empty() {
            return Self::internal_html(
                "Bookmarks",
                r#"<p class="empty">No bookmarks yet. Click the star in the toolbar.</p>"#,
            );
        }
        let mut body = String::from("<ul>");
        for b in list.iter() {
            body.push_str(&format!(
                r#"<li><a href="{}">{}</a><small>{}</small></li>"#,
                esc(&b.url),
                esc(&b.title),
                esc(&host_of(&b.url).unwrap_or_default())
            ));
        }
        body.push_str("</ul>");
        Self::internal_html("Bookmarks", &body)
    }

    fn show_internal(&self, wv: &WebView, page: &str) -> bool {
        let html = match page {
            "history" => self.history_html(),
            "bookmarks" => self.bookmarks_html(),
            _ => return false,
        };
        wv.load_html(&html, Some(&format!("tux://{page}")));
        true
    }
}

// ───────────────────────────── tabs ─────────────────────────────

fn open_tab(browser: &Rc<Browser>, uri: Option<&str>) {
    let is_new_tab = uri.is_none();
    let ucm = UserContentManager::new();
    let filter_on = Rc::new(Cell::new(false));
    if let Some(f) = browser.filter.borrow().as_ref() {
        ucm.add_filter(f);
        filter_on.set(true);
    }
    let fingerprint_script = if browser.prefs.borrow().fingerprint_farbling {
        browser.fingerprint_seed.as_deref().map(|seed| {
            let script = fingerprint_farbling_script(seed);
            ucm.add_script(&script);
            script
        })
    } else {
        None
    };
    let ad_cleaner = UserScript::new(
        AD_CLEANER_SCRIPT,
        UserContentInjectedFrames::AllFrames,
        UserScriptInjectionTime::Start,
        &[],
        &[],
    );
    ucm.add_script(&ad_cleaner);
    if is_new_tab && !ucm.register_script_message_handler("linus", None) {
        eprintln!("Failed to register the new-tab settings message handler");
    }

    let mut builder = WebView::builder().user_content_manager(&ucm);
    if let Some(s) = &browser.session {
        builder = builder.network_session(s);
    }
    let web_view = builder.build();
    web_view.set_hexpand(true);
    web_view.set_vexpand(true);
    if let Some(s) = WebViewExt::settings(&web_view) {
        s.set_enable_developer_extras(true);
        s.set_javascript_can_open_windows_automatically(false);
        s.set_enable_webgl(true);
        s.set_enable_webrtc(true);
    }
    if is_new_tab {
        let weak_browser = Rc::downgrade(browser);
        let weak_view = web_view.downgrade();
        let bridge_disabled = Rc::new(Cell::new(false));
        let bridge_disabled_for_messages = bridge_disabled.clone();
        ucm.connect_script_message_received(Some("linus"), move |_, value| {
            if bridge_disabled_for_messages.get() {
                return;
            }
            let Some(wv) = weak_view.upgrade() else {
                return;
            };
            if wv.uri().is_some_and(|uri| uri.as_str() != "about:blank") {
                return;
            }
            let Some(b) = weak_browser.upgrade() else {
                return;
            };
            let message = value.to_str().to_string();
            if let Some(data_url) = message.strip_prefix("wallpaper:") {
                if data_url == "default" {
                    let path = data_dir().join("new-tab-background");
                    if let Err(error) = fs::remove_file(&path)
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        eprintln!("Could not restore the default new-tab background: {error}");
                    }
                    return;
                }
                if data_url.len() > 14 * 1024 * 1024 {
                    eprintln!("Ignoring new-tab background larger than 10 MB");
                    return;
                }
                let Some((header, encoded)) = data_url.split_once(',') else {
                    eprintln!("Ignoring malformed new-tab background image");
                    return;
                };
                let mime = match header {
                    "data:image/png;base64" => "image/png",
                    "data:image/jpeg;base64" => "image/jpeg",
                    "data:image/webp;base64" => "image/webp",
                    _ => {
                        eprintln!("Ignoring unsupported new-tab background image format");
                        return;
                    }
                };
                let bytes = glib::base64_decode(encoded);
                if bytes.is_empty() || bytes.len() > 10 * 1024 * 1024 {
                    eprintln!("Ignoring invalid or oversized new-tab background image");
                    return;
                }
                if let Err(error) =
                    gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes.clone()))
                {
                    eprintln!("Ignoring invalid new-tab background image: {error}");
                    return;
                }
                let dir = data_dir();
                if let Err(error) = fs::write(dir.join("new-tab-background"), &bytes) {
                    eprintln!("Could not save new-tab background image: {error}");
                    return;
                }
                if let Err(error) = fs::write(dir.join("new-tab-background-mime"), mime) {
                    eprintln!("Could not save new-tab background image type: {error}");
                }
            } else if let Some(value) = message.strip_prefix("wallpaper-style:") {
                if matches!(value, "default" | "custom" | "solid" | "gradient") {
                    let mut prefs = b.prefs.borrow_mut();
                    prefs.wallpaper_style = value.to_string();
                    prefs.save();
                }
            } else if let Some(value) = message.strip_prefix("search:") {
                if matches!(value, "duckduckgo" | "google" | "brave") {
                    let mut prefs = b.prefs.borrow_mut();
                    prefs.search_engine = value.to_string();
                    prefs.save();
                }
            } else if let Some(value) = message.strip_prefix("background:") {
                if matches!(value, "0" | "1") {
                    let mut prefs = b.prefs.borrow_mut();
                    prefs.background_images = value == "1";
                    prefs.save();
                }
            }
        });
        let bridge_manager = ucm.clone();
        let bridge_disabled_on_navigation = bridge_disabled.clone();
        web_view.connect_uri_notify(move |wv| {
            let uri = wv.uri().map(|uri| uri.to_string()).unwrap_or_default();
            if !uri.is_empty()
                && uri != "about:blank"
                && !bridge_disabled_on_navigation.replace(true)
            {
                bridge_manager.unregister_script_message_handler("linus", None);
            }
        });
    }

    let icon = Image::from_paintable(Some(&browser.logo));
    icon.set_pixel_size(16);

    // Let the title use the tab's available width; ellipsize only when the tab
    // itself is narrow, rather than hard-clipping after 10 characters.
    let label = Label::new(Some("New Tab"));
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_max_width_chars(24);
    label.set_hexpand(true);
    label.set_xalign(0.0);

    let close_btn = Button::from_icon_name("window-close-symbolic");
    close_btn.set_has_frame(false);
    close_btn.set_tooltip_text(Some("Close tab (Ctrl+W)"));

    let widget = GtkBox::new(Orientation::Horizontal, 8);
    widget.add_css_class("browser-tab");
    widget.set_hexpand(false);
    widget.set_size_request(-1, -1);
    // GTK CSS cannot express max-width; a 240px width request plus
    // halign=Fill gives tabs a sensible ceiling without stretching.
    widget.set_width_request(240);
    widget.set_halign(Align::Fill);
    widget.append(&icon);
    widget.append(&label);
    widget.append(&close_btn);

    let weak = Rc::downgrade(browser);

    {
        let weak = weak.clone();
        let wv = web_view.clone();
        let click = GestureClick::new();
        click.set_button(0);
        click.connect_pressed(move |g, _, _, _| {
            if let Some(b) = weak.upgrade() {
                match g.current_button() {
                    1 => b.select_tab(&wv),
                    2 => b.close_tab(&wv),
                    _ => {}
                }
            }
        });
        widget.add_controller(click);
    }

    {
        let weak = weak.clone();
        let wv = web_view.clone();
        close_btn.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.close_tab(&wv);
            }
        });
    }

    {
        let weak = weak.clone();
        let label = label.clone();
        web_view.connect_title_notify(move |wv| {
            let title = wv.title().map(|t| t.to_string()).unwrap_or_default();
            let shown = if title.is_empty() { "New Tab" } else { &title };
            label.set_text(shown);
            label.set_tooltip_text(Some(shown));
            if let Some(b) = weak.upgrade() {
                if b.is_current(wv) {
                    b.set_window_title(&title);
                }
            }
        });
    }

    {
        let icon = icon.clone();
        let logo = browser.logo.clone();
        let private = browser.private;
        web_view.connect_favicon_notify(move |wv| match wv.favicon() {
            Some(tex) => {
                icon.set_paintable(Some(&tex));
                if !private {
                    save_history_favicon(wv);
                }
            }
            None => icon.set_paintable(Some(&logo)),
        });
    }

    {
        let weak = weak.clone();
        web_view.connect_uri_notify(move |wv| {
            if let Some(b) = weak.upgrade() {
                if b.is_current(wv) {
                    b.sync_url_for(wv);
                }
            }
        });
    }

    for property in ["can-go-back", "can-go-forward"] {
        let weak = weak.clone();
        web_view.connect_notify_local(Some(property), move |wv, _| {
            if let Some(b) = weak.upgrade() {
                if b.is_current(wv) {
                    b.sync_ui_for(wv);
                }
            }
        });
    }

    {
        let weak = weak.clone();
        web_view.connect_estimated_load_progress_notify(move |wv| {
            if let Some(b) = weak.upgrade() {
                if b.is_current(wv) {
                    b.progress.set_fraction(wv.estimated_load_progress());
                }
            }
        });
    }

    {
        let weak = weak.clone();
        web_view.connect_load_changed(move |wv, event| {
            if let Some(b) = weak.upgrade() {
                if b.is_current(wv) {
                    b.sync_ui_for(wv);
                    if event == LoadEvent::Committed {
                        b.sync_url_for(wv);
                    }
                }
                if event == LoadEvent::Finished {
                    b.record_history(wv);
                }
            }
        });
    }

    // Navigation policy: target=_blank -> new tab, HTTPS upgrade, per-site Shields
    {
        let weak = weak.clone();
        web_view.connect_decide_policy(move |wv, decision, kind| {
            let Some(b) = weak.upgrade() else {
                return false;
            };
            if kind != PolicyDecisionType::NavigationAction
                && kind != PolicyDecisionType::NewWindowAction
            {
                return false;
            }
            let Some(nav) = decision.downcast_ref::<NavigationPolicyDecision>() else {
                return false;
            };
            let Some(action) = nav.navigation_action() else {
                return false;
            };
            let Some(uri) = action
                .request()
                .and_then(|r| r.uri())
                .map(|u| u.to_string())
            else {
                return false;
            };

            if kind == PolicyDecisionType::NavigationAction
                && action.frame_name().is_none()
                && uri == "tux://history/clear"
            {
                decision.ignore();
                let browser = b.clone();
                let web_view = wv.clone();
                glib::idle_add_local_once(move || {
                    browser.clear_history();
                    browser.show_internal(&web_view, "history");
                });
                return true;
            }

            if kind == PolicyDecisionType::NewWindowAction {
                if uri.starts_with("http://") || uri.starts_with("https://") {
                    decision.ignore();
                    let b2 = b.clone();
                    glib::idle_add_local_once(move || open_tab(&b2, Some(&uri)));
                    return true;
                }
                return false;
            }

            if action.frame_name().is_none() {
                if let Some(h) = host_of(&uri) {
                    b.apply_shields_for(wv, &h);
                }
            }

            if let Some(rest) = uri.strip_prefix("http://") {
                let host = host_of(&uri).unwrap_or_default();
                let upgrade = {
                    let p = b.prefs.borrow();
                    p.https_upgrade && !is_local_host(&host)
                };
                if upgrade {
                    let https = format!("https://{rest}");
                    decision.ignore();
                    let wv2 = wv.clone();
                    glib::idle_add_local_once(move || wv2.load_uri(&https));
                    return true;
                }
            }
            false
        });
    }

    browser.tabs.borrow_mut().push(Tab {
        web_view: web_view.clone(),
        widget: widget.clone(),
        label,
        ucm,
        filter_on,
        fingerprint_script,
    });
    browser.tabs_box.append(&widget);
    browser.stack.add_child(&web_view);
    browser.select_tab(&web_view);

    match uri {
        Some(u) => {
            let internal = u.strip_prefix("tux://").map(|p| p.to_string());
            match internal {
                Some(page) if browser.show_internal(&web_view, &page) => {}
                _ => web_view.load_uri(&normalize_input(u, &browser.prefs.borrow().search_engine)),
            }
        }
        None => {
            web_view.load_html(&browser.new_tab_page(), None);
            browser.url_entry.grab_focus();
        }
    }
}

fn install_updated_filter_rules(
    browser: &Rc<Browser>,
    store: &UserContentFilterStore,
    rules: String,
) {
    let weak = Rc::downgrade(browser);
    let store_keep = store.clone();
    store.save(
        "tux-shields",
        &glib::Bytes::from_owned(rules.into_bytes()),
        None::<&gio::Cancellable>,
        move |result| {
            let _keep = &store_keep;
            let Some(browser) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(filter) => {
                    let old_filter = browser.filter.borrow().clone();
                    *browser.filter.borrow_mut() = Some(filter.clone());
                    for tab in browser.tabs.borrow().iter() {
                        if !tab.filter_on.get() {
                            continue;
                        }
                        if let Some(old_filter) = old_filter.as_ref() {
                            tab.ucm.remove_filter(old_filter);
                        }
                        tab.ucm.add_filter(&filter);
                    }
                }
                Err(error) => eprintln!("Could not compile updated Shields rules: {error}"),
            }
        },
    );
}

fn start_filter_list_refresh(browser: &Rc<Browser>, store: UserContentFilterStore) {
    let cache_dir = data_dir().join("filter-lists");
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result =
            refresh_filter_lists(&cache_dir).map(|lists| filter_rules_json_with_lists(&lists));
        if sender.send(result).is_err() {
            eprintln!("Filter-list update result receiver was closed");
        }
    });

    let weak = Rc::downgrade(browser);
    glib::timeout_add_local(Duration::from_millis(250), move || {
        match receiver.try_recv() {
            Ok(Ok(rules)) => {
                if let Some(browser) = weak.upgrade() {
                    install_updated_filter_rules(&browser, &store, rules);
                }
                glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                eprintln!("EasyList/EasyPrivacy update failed: {error}");
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                if weak.upgrade().is_some() {
                    glib::ControlFlow::Continue
                } else {
                    glib::ControlFlow::Break
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                eprintln!("Filter-list update worker stopped without a result");
                glib::ControlFlow::Break
            }
        }
    });
}

// ───────────────────────────── bookmarks, find, downloads ─────────────────────────────

fn rebuild_bookmarks_bar(b: &Rc<Browser>) {
    let bar = &b.bookmarks_bar;
    while let Some(c) = bar.first_child() {
        bar.remove(&c);
    }
    let list = b.bookmarks.borrow().clone();
    let empty = list.is_empty();
    for bm in list {
        let title: String = bm.title.chars().take(22).collect();
        let btn = Button::with_label(&title);
        btn.set_tooltip_text(Some(&bm.url));
        let weak = Rc::downgrade(b);
        let url = bm.url.clone();
        btn.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.navigate(&url);
            }
        });
        bar.append(&btn);
    }
    bar.set_visible(b.prefs.borrow().bookmarks_bar && !empty);
}

fn toggle_bookmark(b: &Rc<Browser>) {
    let Some(wv) = b.current_webview() else {
        return;
    };
    let uri = wv.uri().map(|u| u.to_string()).unwrap_or_default();
    if uri.is_empty() || uri.starts_with("about:") || uri.starts_with("tux://") {
        return;
    }
    let title = wv.title().map(|t| t.to_string()).unwrap_or_default();
    {
        let mut list = b.bookmarks.borrow_mut();
        if let Some(i) = list.iter().position(|x| x.url == uri) {
            list.remove(i);
        } else {
            list.push(Bookmark {
                title: if title.is_empty() { uri.clone() } else { title },
                url: uri.clone(),
            });
        }
        save_bookmarks(&list);
    }
    b.update_bookmark_icon(&uri);
    rebuild_bookmarks_bar(b);
}

fn open_find(b: &Rc<Browser>) {
    b.find_bar.set_visible(true);
    b.find_entry.grab_focus();
}

fn close_find(b: &Rc<Browser>) {
    if let Some(fc) = b.current_webview().and_then(|wv| wv.find_controller()) {
        fc.search_finish();
    }
    b.find_bar.set_visible(false);
    if let Some(wv) = b.current_webview() {
        wv.grab_focus();
    }
}

fn watch_downloads(browser: &Rc<Browser>) {
    let Some(session) = browser.net_session() else {
        return;
    };
    let weak = Rc::downgrade(browser);
    session.connect_download_started(move |_, download| {
        let Some(b) = weak.upgrade() else { return };

        let row = GtkBox::new(Orientation::Horizontal, 12);
        let name = Label::new(Some("Downloading…"));
        name.set_hexpand(true);
        name.set_xalign(0.0);
        name.set_ellipsize(pango::EllipsizeMode::Middle);
        name.set_max_width_chars(32);
        let status = Label::new(Some("0%"));
        row.append(&name);
        row.append(&status);
        b.downloads_list.append(&row);
        b.downloads_btn.set_visible(true);

        {
            let name = name.clone();
            download.connect_decide_destination(move |dl, suggested| {
                let dir = glib::user_special_dir(glib::UserDirectory::Downloads)
                    .unwrap_or_else(glib::home_dir);
                let path = unique_path(&dir, suggested);
                name.set_text(
                    &path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                );
                dl.set_destination(&path.to_string_lossy());
                true
            });
        }
        {
            let status = status.clone();
            download.connect_received_data(move |dl, _| {
                status.set_text(&format!("{:.0}%", dl.estimated_progress() * 100.0));
            });
        }
        {
            let status = status.clone();
            download.connect_finished(move |_| status.set_text("Done"));
        }
        {
            let status = status.clone();
            download.connect_failed(move |_, _| status.set_text("Failed"));
        }
    });
}

// ───────────────────────────── wiring helpers ─────────────────────────────

fn on_click(button: &Button, browser: &Rc<Browser>, f: impl Fn(&Rc<Browser>) + 'static) {
    let weak = Rc::downgrade(browser);
    button.connect_clicked(move |_| {
        if let Some(b) = weak.upgrade() {
            f(&b);
        }
    });
}

fn add_action(
    app: &Application,
    browser: &Rc<Browser>,
    name: &str,
    accels: &[&str],
    f: impl Fn(&Rc<Browser>) + 'static,
) {
    let weak = Rc::downgrade(browser);
    let action = gio::SimpleAction::new(name, None);
    action.connect_activate(move |_, _| {
        if let Some(b) = weak.upgrade() {
            f(&b);
        }
    });
    browser.window.add_action(&action);
    app.set_accels_for_action(&format!("win.{name}"), accels);
}

fn zoom(b: &Rc<Browser>, delta: f64) {
    if let Some(wv) = b.current_webview() {
        let z = if delta == 0.0 {
            1.0
        } else {
            (wv.zoom_level() + delta).clamp(0.3, 5.0)
        };
        wv.set_zoom_level(z);
    }
}

// ───────────────────────────── window ─────────────────────────────

fn build_window(app: &Application, private: bool) {
    // `gtk-application-prefer-dark-theme` is deprecated (it logs a warning at
    // startup). The whole chrome is themed by our own stylesheet, so the theme
    // preference is unnecessary; only tell GTK which accent scheme to assume for
    // stock widgets we do not draw ourselves.
    let theme_monitors = load_css();

    let logo = gdk::Texture::from_bytes(&glib::Bytes::from_static(TUX_PNG))
        .expect("embedded logo is not a valid image");
    let logo_b64 = glib::base64_encode(TUX_PNG).to_string();

    let window = ApplicationWindow::builder()
        .application(app)
        .title(APP_TITLE)
        .default_width(1340)
        .default_height(760)
        .build();
    if private {
        window.add_css_class("private");
    }

    // ---- tab strip ----
    // The tab box must expand, otherwise tabs pile up on the left and push the
    // trailing controls (tab list, close) past the window edge.
    let tabs_box = GtkBox::new(Orientation::Horizontal, 0);
    tabs_box.set_hexpand(true);
    tabs_box.set_halign(Align::Start);
    tabs_box.set_valign(Align::Center);

    let new_tab_btn = Button::from_icon_name("list-add-symbolic");
    new_tab_btn.set_tooltip_text(Some("New tab (Ctrl+T)"));
    new_tab_btn.add_css_class("tabbar-button");
    new_tab_btn.set_has_frame(false);
    new_tab_btn.set_valign(Align::Center);

    let list_btn = MenuButton::builder()
        .icon_name("pan-down-symbolic")
        .tooltip_text("Search tabs")
        .build();
    list_btn.add_css_class("tabbar-button");
    list_btn.set_has_frame(false);
    list_btn.set_valign(Align::Center);
    let list_pop = Popover::new();
    let list_box = GtkBox::new(Orientation::Vertical, 2);
    list_pop.set_child(Some(&list_box));
    list_btn.set_popover(Some(&list_pop));

    let win_close = Button::from_icon_name("window-close-symbolic");
    win_close.set_tooltip_text(Some("Close window"));
    win_close.add_css_class("tabbar-close");
    win_close.set_has_frame(false);
    win_close.set_valign(Align::Center);

    // Trailing controls are pinned to the end so they can never be pushed out.
    let trailing = GtkBox::new(Orientation::Horizontal, 2);
    trailing.set_valign(Align::Center);
    trailing.append(&list_btn);
    trailing.append(&win_close);

    let tabbar = GtkBox::new(Orientation::Horizontal, 4);
    tabbar.add_css_class("tabbar");
    tabbar.append(&tabs_box);
    tabbar.append(&new_tab_btn);
    tabbar.append(&trailing);

    let handle = WindowHandle::new();
    handle.add_css_class("tabbar-handle");
    handle.set_child(Some(&tabbar));
    window.set_titlebar(Some(&handle));

    // ---- toolbar ----
    let back = Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some("Back (Alt+←)"));
    back.set_has_frame(false);
    let forward = Button::from_icon_name("go-next-symbolic");
    forward.set_tooltip_text(Some("Forward (Alt+→)"));
    forward.set_has_frame(false);
    let reload = Button::from_icon_name("view-refresh-symbolic");
    reload.set_tooltip_text(Some("Reload (Ctrl+R)"));
    reload.set_has_frame(false);

    let bookmark = Button::from_icon_name("non-starred-symbolic");
    bookmark.set_tooltip_text(Some("Bookmark this page (Ctrl+D)"));
    bookmark.set_has_frame(false);

    let url_entry = Entry::builder()
        .hexpand(true)
        .placeholder_text("Search or enter address")
        .build();
    url_entry.add_css_class("url-entry");
    url_entry.set_icon_from_paintable(EntryIconPosition::Primary, Some(&logo));
    url_entry.set_icon_activatable(EntryIconPosition::Primary, false);

    // Shields popover (opened from the main menu). It must NOT be parented to
    // the URL entry at build time: a parented popover is realized as part of
    // the toolbar and its children painted inside the address pill.
    let shields_pop = Popover::new();
    let shields_box = GtkBox::new(Orientation::Vertical, 10);
    shields_box.set_margin_top(12);
    shields_box.set_margin_bottom(12);
    shields_box.set_margin_start(14);
    shields_box.set_margin_end(14);
    shields_box.set_width_request(300);
    let shields_title = Label::new(Some("Shields"));
    shields_title.add_css_class("title-4");
    shields_title.set_xalign(0.0);
    let shields_host = Label::new(Some("No site loaded"));
    shields_host.add_css_class("dim-label");
    shields_host.set_xalign(0.0);
    let fingerprint_status = Label::new(Some(FINGERPRINT_STATUS_ACTIVE));
    fingerprint_status.add_css_class("warning");
    fingerprint_status.set_xalign(0.0);
    let fingerprint_info = Label::new(Some(FINGERPRINT_INFO));
    fingerprint_info.add_css_class("dim-label");
    fingerprint_info.set_wrap(true);
    fingerprint_info.set_xalign(0.0);
    let (row_fingerprint, fingerprint_sw) =
        switch_row("Randomize fingerprint signals (experimental)");
    let (row_site, shields_sw) = switch_row("Shields up for this site");
    let (row_https, https_sw) = switch_row("Upgrade connections to HTTPS");
    let (row_cookies, cookies_sw) = switch_row("Block third-party cookies");
    let shields_info = Label::new(Some(
        "Known third-party ad and tracker domains are blocked while Shields are up.",
    ));
    shields_info.add_css_class("dim-label");
    shields_info.set_wrap(true);
    shields_info.set_xalign(0.0);
    shields_box.append(&shields_title);
    shields_box.append(&shields_host);
    shields_box.append(&row_site);
    shields_box.append(&row_https);
    shields_box.append(&row_cookies);
    shields_box.append(&row_fingerprint);
    shields_box.append(&fingerprint_status);
    shields_box.append(&fingerprint_info);
    shields_box.append(&shields_info);
    shields_pop.set_child(Some(&shields_box));

    // Downloads (hidden until the first download)
    let downloads_btn = MenuButton::builder()
        .icon_name("document-save-symbolic")
        .tooltip_text("Downloads")
        .has_frame(false)
        .visible(false)
        .build();
    let downloads_pop = Popover::new();
    let downloads_list = GtkBox::new(Orientation::Vertical, 8);
    downloads_list.set_margin_top(10);
    downloads_list.set_margin_bottom(10);
    downloads_list.set_margin_start(12);
    downloads_list.set_margin_end(12);
    downloads_list.set_width_request(320);
    downloads_pop.set_child(Some(&downloads_list));
    downloads_btn.set_popover(Some(&downloads_pop));

    let extensions = Button::from_icon_name("application-x-addon-symbolic");
    extensions.set_tooltip_text(Some("Extensions (not supported by WebKit)"));
    extensions.set_sensitive(false);
    extensions.set_has_frame(false);

    let menu = gio::Menu::new();
    let s1 = gio::Menu::new();
    s1.append(Some("New tab"), Some("win.new-tab"));
    s1.append(Some("New window"), Some("win.new-window"));
    s1.append(Some("New private window"), Some("win.new-private"));
    let s2 = gio::Menu::new();
    s2.append(Some("Bookmarks"), Some("win.bookmarks"));
    s2.append(Some("History"), Some("win.history"));
    s2.append(Some("Downloads"), Some("win.downloads"));
    s2.append(Some("Privacy and Shields"), Some("win.privacy"));
    let s3 = gio::Menu::new();
    s3.append(Some("Show bookmarks bar"), Some("win.toggle-bar"));
    s3.append(Some("Find in page…"), Some("win.find"));
    s3.append(Some("Zoom in"), Some("win.zoom-in"));
    s3.append(Some("Zoom out"), Some("win.zoom-out"));
    s3.append(Some("Actual size"), Some("win.zoom-reset"));
    s3.append(Some("Full screen"), Some("win.fullscreen"));
    s3.append(Some("Developer tools"), Some("win.devtools"));
    let s4 = gio::Menu::new();
    s4.append(Some("Clear browsing history"), Some("win.clear-history"));
    s4.append(Some("Quit"), Some("win.quit"));
    menu.append_section(None, &s1);
    menu.append_section(None, &s2);
    menu.append_section(None, &s3);
    menu.append_section(None, &s4);
    let menu_btn = MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text("Menu")
        .has_frame(false)
        .build();
    // The Shields popover used to be parented to the URL entry and then moved
    // onto the main menu button, which already owns a menu model. That left it
    // orphaned and it rendered as a stray floating island inside the toolbar.
    // It is opened explicitly from the Shields menu item instead, so keep it
    // hidden and off the toolbar until then.
    shields_pop.unparent();
    shields_pop.set_visible(false);

    // Group the toolbar into navigation / address / actions. Explicit groups
    // keep spacing even and stop the icon cluster from crowding the address
    // pill, which previously relied on a magic margin.
    let nav_group = GtkBox::new(Orientation::Horizontal, 0);
    nav_group.add_css_class("toolbar-group");
    nav_group.append(&back);
    nav_group.append(&forward);
    nav_group.append(&reload);

    let action_group = GtkBox::new(Orientation::Horizontal, 0);
    action_group.add_css_class("toolbar-group");
    action_group.append(&bookmark);
    action_group.append(&downloads_btn);
    action_group.append(&extensions);
    action_group.append(&menu_btn);

    let toolbar = GtkBox::new(Orientation::Horizontal, 6);
    toolbar.add_css_class("toolbar-row");
    toolbar.set_hexpand(true);
    toolbar.set_valign(Align::Center);
    toolbar.append(&nav_group);
    toolbar.append(&url_entry);
    toolbar.append(&action_group);

    let bookmarks_bar = GtkBox::new(Orientation::Horizontal, 2);
    bookmarks_bar.add_css_class("bookmarks-bar");

    let progress = ProgressBar::new();
    progress.add_css_class("load-progress");
    progress.set_visible(false);

    let stack = Stack::new();
    stack.set_vexpand(true);
    stack.set_hexpand(true);

    // ---- find bar ----
    let find_entry = SearchEntry::new();
    find_entry.set_hexpand(true);
    find_entry.set_placeholder_text(Some("Find in page"));
    let find_prev = Button::from_icon_name("go-up-symbolic");
    let find_next = Button::from_icon_name("go-down-symbolic");
    let find_close = Button::from_icon_name("window-close-symbolic");
    let find_bar = GtkBox::new(Orientation::Horizontal, 6);
    find_bar.add_css_class("find-bar");
    find_bar.append(&find_entry);
    find_bar.append(&find_prev);
    find_bar.append(&find_next);
    find_bar.append(&find_close);
    find_bar.set_visible(false);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.append(&toolbar);
    content.append(&bookmarks_bar);
    content.append(&progress);
    content.append(&stack);
    content.append(&find_bar);
    window.set_child(Some(&content));

    let prefs = Prefs::load();
    let fingerprint_seed = match new_fingerprint_seed() {
        Ok(seed) => Some(seed),
        Err(error) => {
            eprintln!("Could not create session fingerprint seed: {error}");
            None
        }
    };
    https_sw.set_active(prefs.https_upgrade);
    cookies_sw.set_active(prefs.block_cookies);
    fingerprint_sw.set_sensitive(fingerprint_seed.is_some());
    fingerprint_sw.set_active(prefs.fingerprint_farbling && fingerprint_seed.is_some());

    let session = if private {
        Some(NetworkSession::new_ephemeral())
    } else {
        NetworkSession::default()
    };
    if let Some(session) = &session {
        session.set_itp_enabled(true);
        session.set_tls_errors_policy(TLSErrorsPolicy::Fail);
    }
    apply_cookie_policy(
        session.clone().or_else(NetworkSession::default),
        prefs.block_cookies,
    );

    let browser = Rc::new(Browser {
        window: window.clone(),
        app: app.clone(),
        private,
        session,
        stack,
        tabs_box,
        tabs: RefCell::new(Vec::new()),
        url_entry: url_entry.clone(),
        back: back.clone(),
        forward: forward.clone(),
        reload: reload.clone(),
        bookmark: bookmark.clone(),
        progress,
        bookmarks_bar,
        find_bar,
        find_entry: find_entry.clone(),
        shields_pop: shields_pop.clone(),
        menu_btn: menu_btn.clone(),
        shields_sw: shields_sw.clone(),
        shields_host,
        fingerprint_sw: fingerprint_sw.clone(),
        fingerprint_status,
        fingerprint_seed,
        syncing: Cell::new(false),
        downloads_btn: downloads_btn.clone(),
        downloads_list,
        logo,
        logo_b64,
        filter: RefCell::new(None),
        prefs: RefCell::new(prefs),
        bookmarks: RefCell::new(load_bookmarks()),
        history: RefCell::new(if private { Vec::new() } else { load_history() }),
        _theme_monitors: theme_monitors,
    });

    rebuild_bookmarks_bar(&browser);
    watch_downloads(&browser);

    // ---- buttons ----
    on_click(&back, &browser, |b| {
        if let Some(wv) = b.current_webview() {
            wv.go_back();
        }
    });
    on_click(&forward, &browser, |b| {
        if let Some(wv) = b.current_webview() {
            wv.go_forward();
        }
    });
    on_click(&reload, &browser, |b| {
        if let Some(wv) = b.current_webview() {
            if wv.is_loading() {
                wv.stop_loading();
            } else {
                b.reload_current();
            }
        }
    });
    on_click(&new_tab_btn, &browser, |b| open_tab(b, None));
    on_click(&win_close, &browser, |b| b.window.close());
    on_click(&bookmark, &browser, toggle_bookmark);
    on_click(&find_close, &browser, close_find);
    on_click(&find_next, &browser, |b| {
        if let Some(fc) = b.current_webview().and_then(|wv| wv.find_controller()) {
            fc.search_next();
        }
    });
    on_click(&find_prev, &browser, |b| {
        if let Some(fc) = b.current_webview().and_then(|wv| wv.find_controller()) {
            fc.search_previous();
        }
    });

    // ---- address bar ----
    {
        let weak = Rc::downgrade(&browser);
        url_entry.connect_activate(move |entry| {
            if let Some(b) = weak.upgrade() {
                b.navigate(entry.text().as_str());
                if let Some(wv) = b.current_webview() {
                    wv.grab_focus();
                }
            }
        });
    }

    // ---- shields switches ----
    {
        let weak = Rc::downgrade(&browser);
        shields_sw.connect_active_notify(move |sw| {
            let Some(b) = weak.upgrade() else { return };
            if b.syncing.get() {
                return;
            }
            let Some(wv) = b.current_webview() else {
                return;
            };
            let Some(host) = wv.uri().and_then(|u| host_of(u.as_str())) else {
                return;
            };
            {
                let mut p = b.prefs.borrow_mut();
                if sw.is_active() {
                    p.shields_off.remove(&host);
                } else {
                    p.shields_off.insert(host.clone());
                }
                p.save();
            }
            b.apply_shields_for(&wv, &host);
            b.sync_shields_ui(wv.uri().map(|u| u.to_string()).unwrap_or_default().as_str());
            wv.reload();
        });
    }
    {
        let weak = Rc::downgrade(&browser);
        fingerprint_sw.connect_active_notify(move |sw| {
            let Some(b) = weak.upgrade() else { return };
            if b.syncing.get() {
                return;
            }
            let enabled = sw.is_active() && b.fingerprint_seed.is_some();
            {
                let mut prefs = b.prefs.borrow_mut();
                prefs.fingerprint_farbling = enabled;
                prefs.save();
            }
            b.apply_fingerprint_farbling(enabled);
            if let Some(wv) = b.current_webview() {
                b.sync_shields_ui(
                    wv.uri()
                        .map(|uri| uri.to_string())
                        .unwrap_or_default()
                        .as_str(),
                );
            }
        });
    }
    {
        let weak = Rc::downgrade(&browser);
        https_sw.connect_active_notify(move |sw| {
            if let Some(b) = weak.upgrade() {
                let mut p = b.prefs.borrow_mut();
                p.https_upgrade = sw.is_active();
                p.save();
            }
        });
    }
    {
        let weak = Rc::downgrade(&browser);
        cookies_sw.connect_active_notify(move |sw| {
            if let Some(b) = weak.upgrade() {
                {
                    let mut p = b.prefs.borrow_mut();
                    p.block_cookies = sw.is_active();
                    p.save();
                }
                apply_cookie_policy(b.net_session(), sw.is_active());
            }
        });
    }

    // ---- find in page ----
    {
        let weak = Rc::downgrade(&browser);
        find_entry.connect_search_changed(move |e| {
            let Some(b) = weak.upgrade() else { return };
            let Some(fc) = b.current_webview().and_then(|wv| wv.find_controller()) else {
                return;
            };
            let text = e.text();
            if text.is_empty() {
                fc.search_finish();
            } else {
                let opts = (FindOptions::CASE_INSENSITIVE | FindOptions::WRAP_AROUND).bits();
                fc.search(text.as_str(), opts, 1000);
            }
        });
    }
    {
        let weak = Rc::downgrade(&browser);
        find_entry.connect_activate(move |_| {
            if let Some(fc) = weak
                .upgrade()
                .and_then(|b| b.current_webview())
                .and_then(|wv| wv.find_controller())
            {
                fc.search_next();
            }
        });
    }
    {
        let weak = Rc::downgrade(&browser);
        find_entry.connect_stop_search(move |_| {
            if let Some(b) = weak.upgrade() {
                close_find(&b);
            }
        });
    }

    // ---- tab list dropdown ----
    {
        let weak = Rc::downgrade(&browser);
        let list_box = list_box.clone();
        let pop_weak = list_pop.downgrade();
        list_pop.connect_show(move |_| {
            let Some(b) = weak.upgrade() else { return };
            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }
            for t in b.tabs.borrow().iter() {
                let btn = Button::with_label(&t.label.text());
                btn.set_has_frame(false);
                if let Some(l) = btn.child().and_then(|c| c.downcast::<Label>().ok()) {
                    l.set_xalign(0.0);
                    l.set_ellipsize(pango::EllipsizeMode::End);
                    l.set_max_width_chars(40);
                }
                let wv = t.web_view.clone();
                let weak = weak.clone();
                let pop_weak = pop_weak.clone();
                btn.connect_clicked(move |_| {
                    if let Some(p) = pop_weak.upgrade() {
                        p.popdown();
                    }
                    if let Some(b) = weak.upgrade() {
                        b.select_tab(&wv);
                    }
                });
                list_box.append(&btn);
            }
        });
    }

    // ---- actions & shortcuts ----
    add_action(app, &browser, "new-tab", &["<Primary>t"], |b| {
        open_tab(b, None)
    });
    add_action(app, &browser, "close-tab", &["<Primary>w"], |b| {
        if let Some(wv) = b.current_webview() {
            b.close_tab(&wv);
        }
    });
    add_action(app, &browser, "new-window", &["<Primary>n"], |b| {
        build_window(&b.app, false)
    });
    add_action(app, &browser, "new-private", &["<Primary><Shift>n"], |b| {
        build_window(&b.app, true)
    });
    add_action(app, &browser, "quit", &["<Primary>q"], |b| b.app.quit());
    add_action(app, &browser, "focus-url", &["<Primary>l", "F6"], |b| {
        b.url_entry.grab_focus();
    });
    add_action(app, &browser, "reload", &["<Primary>r", "F5"], |b| {
        b.reload_current()
    });
    add_action(app, &browser, "stop", &["Escape"], |b| {
        if let Some(wv) = b.current_webview() {
            wv.stop_loading();
        }
    });
    add_action(app, &browser, "back", &["<Alt>Left"], |b| {
        if let Some(wv) = b.current_webview() {
            wv.go_back();
        }
    });
    add_action(app, &browser, "forward", &["<Alt>Right"], |b| {
        if let Some(wv) = b.current_webview() {
            wv.go_forward();
        }
    });
    add_action(
        app,
        &browser,
        "next-tab",
        &["<Primary>Tab", "<Primary>Page_Down"],
        |b| b.cycle_tab(1),
    );
    add_action(
        app,
        &browser,
        "prev-tab",
        &["<Primary><Shift>Tab", "<Primary>Page_Up"],
        |b| b.cycle_tab(-1),
    );
    add_action(
        app,
        &browser,
        "bookmark-page",
        &["<Primary>d"],
        toggle_bookmark,
    );
    add_action(app, &browser, "bookmarks", &["<Primary><Shift>o"], |b| {
        open_tab(b, Some("tux://bookmarks"))
    });
    add_action(app, &browser, "history", &["<Primary>h"], |b| {
        open_tab(b, Some("tux://history"))
    });
    add_action(app, &browser, "downloads", &["<Primary>j"], |b| {
        b.downloads_btn.set_visible(true);
        b.downloads_btn.popup();
    });
    // Shields is parented on demand, under the toolbar, when the menu item is
    // chosen — never at build time, so it cannot leak into the toolbar layout.
    add_action(app, &browser, "privacy", &[], |b| {
        b.shields_pop.set_parent(&b.menu_btn);
        b.shields_pop.set_visible(true);
        b.shields_pop.popup();
    });
    add_action(app, &browser, "toggle-bar", &["<Primary><Shift>b"], |b| {
        {
            let mut p = b.prefs.borrow_mut();
            p.bookmarks_bar = !p.bookmarks_bar;
            p.save();
        }
        rebuild_bookmarks_bar(b);
    });
    add_action(app, &browser, "find", &["<Primary>f"], open_find);
    add_action(
        app,
        &browser,
        "zoom-in",
        &["<Primary>plus", "<Primary>equal", "<Primary>KP_Add"],
        |b| zoom(b, 0.1),
    );
    add_action(
        app,
        &browser,
        "zoom-out",
        &["<Primary>minus", "<Primary>KP_Subtract"],
        |b| zoom(b, -0.1),
    );
    add_action(app, &browser, "zoom-reset", &["<Primary>0"], |b| {
        zoom(b, 0.0)
    });
    add_action(app, &browser, "fullscreen", &["F11"], |b| {
        if b.window.is_fullscreen() {
            b.window.unfullscreen();
        } else {
            b.window.fullscreen();
        }
    });
    add_action(
        app,
        &browser,
        "devtools",
        &["F12", "<Primary><Shift>i"],
        |b| {
            if let Some(i) = b.current_webview().and_then(|wv| wv.inspector()) {
                i.show();
            }
        },
    );
    add_action(
        app,
        &browser,
        "clear-history",
        &["<Primary><Shift>Delete"],
        |b| {
            b.clear_history();
            if let Some(wv) = b.current_webview() {
                if wv
                    .uri()
                    .map(|u| u.starts_with("tux://history"))
                    .unwrap_or(false)
                {
                    b.show_internal(&wv, "history");
                }
            }
        },
    );

    // Keep the browser alive for the window's lifetime; unparent the popover on close.
    {
        let keep_alive = browser.clone();
        window.connect_close_request(move |_| {
            keep_alive.shields_pop.unparent();
            glib::Propagation::Proceed
        });
    }

    // Compile the tracker filter first so the very first page is already protected.
    {
        let rules = filter_rules_json_with_lists(&FilterListDomains::default());
        let store = UserContentFilterStore::new(&data_dir().join("filters").to_string_lossy());
        let store_keep = store.clone();
        let weak = Rc::downgrade(&browser);
        store.save(
            "tux-shields",
            &glib::Bytes::from_owned(rules.into_bytes()),
            None::<&gio::Cancellable>,
            move |result| {
                let _keep = &store_keep;
                if let Some(b) = weak.upgrade() {
                    match result {
                        Ok(f) => *b.filter.borrow_mut() = Some(f),
                        Err(e) => eprintln!("Could not compile Shields rules: {e}"),
                    }
                    open_tab(&b, None);
                    start_filter_list_refresh(&b, store_keep.clone());
                }
            },
        );
    }

    window.present();
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("com.silentunite.linusbrowser")
        .build();
    app.connect_activate(|app| build_window(app, false));
    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tab_customize_panel_has_opaque_modal_and_synchronized_backdrop() {
        assert!(NEW_TAB_HTML.contains("background: #21191a"));
        assert!(
            NEW_TAB_HTML
                .contains(".nav.active { color: #ffb1a3; background: rgba(255,111,85,.13); }")
        );
        assert!(NEW_TAB_HTML.contains(".nav.active::before"));
        assert!(NEW_TAB_HTML.contains("background: #ffaaa0; border: 3px solid #fff"));
        assert!(NEW_TAB_HTML.contains("border: solid #fff; border-width: 0 3px 3px 0"));
        assert!(NEW_TAB_HTML.contains(".card.selected .select-mark { display: grid; }"));
        assert!(NEW_TAB_HTML.contains("class=\"toggle-switch\""));
        assert!(NEW_TAB_HTML.contains("class=\"switch-track\""));
        assert!(NEW_TAB_HTML.contains("class=\"customize-backdrop\""));
        assert!(NEW_TAB_HTML.contains("aria-modal=\"true\""));
        assert!(NEW_TAB_HTML.contains("customizeBackdrop.classList.add('open')"));
        assert!(NEW_TAB_HTML.contains("customizeBackdrop.classList.remove('open')"));
        assert!(NEW_TAB_HTML.contains("content: \"+\\A Choose an image\""));
    }

    #[test]
    fn new_tab_search_panel_lists_supported_engines_as_persistent_radio_choices() {
        assert!(NEW_TAB_HTML.contains("role=\"radiogroup\" aria-label=\"Default search engine\""));
        for engine in ["duckduckgo", "google", "brave"] {
            assert!(NEW_TAB_HTML.contains(&format!("value=\"{engine}\"")));
        }
        assert!(NEW_TAB_HTML.contains("src=\"{{DUCKDUCKGO_LOGO}}\""));
        assert!(NEW_TAB_HTML.contains("src=\"{{GOOGLE_LOGO}}\""));
        assert!(NEW_TAB_HTML.contains("src=\"{{BRAVE_LOGO}}\""));
        assert!(!DUCKDUCKGO_SEARCH_ICON.is_empty());
        assert!(!GOOGLE_SEARCH_ICON.is_empty());
        assert!(!BRAVE_SEARCH_ICON.is_empty());
        assert!(NEW_TAB_HTML.contains("send('search:' + input.value);"));
        assert!(NEW_TAB_HTML.contains("input[name=\"search-engine\"]:checked"));
    }

    #[test]
    fn history_page_supports_grouped_visits_search_and_confirmed_clear() {
        assert!(HISTORY_PAGE.contains("id=\"history-search\""));
        assert!(HISTORY_PAGE.contains("querySelectorAll('.day-group')"));
        assert!(HISTORY_PAGE.contains("visit.dataset.search"));
        assert!(HISTORY_PAGE.contains("No matching pages found."));
        assert!(HISTORY_PAGE.contains("window.confirm('Clear all browsing history?')"));
        assert!(HISTORY_PAGE.contains("tux://history/clear"));
        assert!(HISTORY_PAGE.contains("{{HISTORY_COUNT}}"));
        assert!(HISTORY_PAGE.contains("{{HISTORY_CONTENT}}"));
    }

    #[test]
    fn history_uses_cached_site_favicons_and_origin_fallback() {
        assert!(HISTORY_PAGE.contains(".site-icon img {"));
        assert!(HISTORY_PAGE.contains(".site-icon img[hidden] {"));
        let icon = history_site_icon_html("https://example.com/favicon.ico");
        assert!(icon.contains("class=\"site-fallback\""));
        assert!(icon.contains("class=\"site-favicon\""));
        assert!(icon.contains("onload=\"this.previousElementSibling.hidden=true\""));
        assert!(icon.contains(glib::base64_encode(TUX_PNG).as_str()));
        assert!(icon.contains("https://example.com/favicon.ico"));
        assert_eq!(
            history_favicon_src("https://favicon-cache-test.invalid:8443/path"),
            "https://favicon-cache-test.invalid:8443/favicon.ico"
        );
    }

    #[test]
    fn wallpaper_palette_extraction_generates_brave_chrome_tokens() {
        // Strongly red wallpaper pixels drive the tint.
        let pixels = [
            0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 128, 0, 255,
        ];
        let palette = palette_css_from_bgra(4, 1, 16, &pixels).expect("valid pixel palette");
        // The wallpaper may only tint the dark ramp, never flatten it: every
        // surface token must be present and the shell must stay dark.
        for token in ["shell", "chrome", "surface", "surface_hi"] {
            assert!(
                palette.contains(&format!("@define-color {token} rgb(")),
                "missing surface token @{token}"
            );
        }
        assert!(palette.contains("@define-color window_bg @shell;"));
        assert!(palette.contains("@define-color focus_ring rgba("));
        // Every surface stays dark (all channels well under mid-grey),
        // regardless of how bright the wallpaper is.
        for line in palette.lines().filter(|line| {
            ["shell", "chrome", "surface", "surface_hi"]
                .iter()
                .any(|token| line.contains(&format!("@define-color {token} rgb(")))
        }) {
            let numbers: Vec<u32> = line
                .split_whitespace()
                .filter_map(|part| {
                    part.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok()
                })
                .collect();
            assert!(
                numbers.iter().all(|channel| *channel < 90),
                "surface token is too light (would flatten the chrome): {line}"
            );
        }
        assert!(palette_css_from_bgra(1, 1, 4, &[0, 0, 0]).is_err());
    }

    #[test]
    fn wallpaper_accent_falls_back_to_brave_orange_when_muted() {
        // A near-black wallpaper would previously yield something like #370000,
        // making focus rings and the progress bar invisible.
        let dark = palette_css_from_bgra(1, 1, 4, &[10, 8, 6, 255]).expect("palette");
        assert!(
            dark.contains("@define-color accent_color #ff6a4d;"),
            "muted wallpapers must fall back to the design accent, got:\n{dark}"
        );
        assert!(dark.contains("@define-color focus_ring rgba(255, 106, 77, 0.35);"));

        // A vivid wallpaper still supplies the accent.
        let vivid = palette_css_from_bgra(1, 1, 4, &[30, 120, 255, 255]).expect("palette");
        assert!(
            !vivid.contains("@define-color accent_color #ff6a4d;"),
            "vivid wallpapers should keep their own accent, got:\n{vivid}"
        );
    }

    #[test]
    fn dynamic_theme_css_keeps_base_palette_and_user_overrides_and_uses_watch_path() {
        let css = composed_theme_css(
            "@define-color accent_color #123456;",
            "@define-color accent_color #abcdef;",
        );
        assert!(css.starts_with(CSS));
        assert!(
            css.find("@define-color accent_color #123456;")
                < css.find("@define-color accent_color #abcdef;")
        );
        assert!(theme_css_path().ends_with("linus_browser/theme.css"));
        assert!(CSS.contains("window, window.background { background-color: @window_bg;"));
        assert!(CSS.contains("@define-color window_bg @shell;"));
    }

    #[test]
    fn restyled_theme_declares_design_tokens_and_remains_valid_gtk_css() {
        // Design tokens the rest of the sheet is built from must be declared.
        for token in [
            "shell",
            "chrome",
            "surface",
            "surface_hi",
            "canvas",
            "fg_dim",
            "fg_mute",
            "hairline",
            "hairline_strong",
            "focus_ring",
            "danger",
            "success",
            "warning",
            "popover_bg",
            "field_mute",
        ] {
            assert!(
                CSS.contains(&format!("@define-color {token} ")),
                "missing design token @{token}"
            );
        }

        // No stray GTK focus rectangles: every interactive rule opts out.
        assert!(CSS.contains("*:focus-visible { outline: none; }"));

        // The address bar is a borderless white pill; focus never adds a ring.
        assert!(CSS.contains("entry.url-entry:focus-within"));
        assert!(CSS.contains("border-radius: 999px"));
        let url_entry_rule = CSS
            .split("entry.url-entry:focus, entry.url-entry:focus-within {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("address bar focus rule");
        assert!(
            !url_entry_rule.contains("border-color") && !url_entry_rule.contains("0 0 0"),
            "address bar focus must not draw an outline/ring: {url_entry_rule}"
        );

        // Tabs are flat; only the raised active surface differs.
        assert!(CSS.contains(".browser-tab.active {"));
        assert!(!CSS.contains(".browser-tab.active::after"));

        // Popovers and the private theme.
        assert!(CSS.contains("popover.background, popover > contents"));
        assert!(CSS.contains("window.private .toolbar-row"));

        // The stylesheet must be syntactically well formed (balanced braces and
        // parseable declarations). This checks structure without needing a display.
        let composed = composed_theme_css(
            &palette_css_from_bgra(1, 1, 4, &[0, 0, 255, 255]).expect("palette"),
            "",
        );
        let mut depth = 0_i32;
        let mut minimum = 0_i32;
        for ch in composed.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    minimum = minimum.min(depth);
                }
                _ => {}
            }
        }
        assert_eq!(depth, 0, "unbalanced braces in theme CSS");
        assert_eq!(minimum, 0, "stray closing brace in theme CSS");
        assert!(
            !composed.contains("alpha(@") && !composed.contains("::after {"),
            "theme CSS uses constructs GTK CSS does not support"
        );
    }

    #[test]
    fn new_tab_background_gallery_offers_and_applies_all_four_styles() {
        assert!(NEW_TAB_HTML.contains("send('wallpaper-style:' + style);"));
        assert!(NEW_TAB_HTML.contains("body.wallpaper-solid"));
        assert!(NEW_TAB_HTML.contains("body.wallpaper-gradient"));
        assert!(NEW_TAB_HTML.contains("id=\"default-background\""));
        assert!(NEW_TAB_HTML.contains("id=\"choose-background\""));
        assert!(NEW_TAB_HTML.contains("id=\"solid-background\""));
        assert!(NEW_TAB_HTML.contains("id=\"gradient-background\""));
        assert_eq!(effective_wallpaper_style("default", true), "default");
        assert_eq!(effective_wallpaper_style("custom", false), "default");
        assert_eq!(effective_wallpaper_style("custom", true), "custom");
        assert_eq!(effective_wallpaper_style("gradient", true), "gradient");
    }

    #[test]
    fn fingerprint_farbling_script_changes_canvas_and_masks_webgl_renderer() {
        let script = FINGERPRINT_FARBLING_SCRIPT.replace("__SESSION_SEED__", "test-session-seed");

        assert!(script.contains("CanvasRenderingContext2D.prototype"));
        assert!(script.contains("HTMLCanvasElement.prototype.toDataURL"));
        assert!(script.contains("HTMLCanvasElement.prototype.toBlob"));
        assert!(script.contains("prototype.readPixels"));
        assert!(script.contains("WebKit WebGL"));
        assert!(!script.contains("__SESSION_SEED__"));
    }

    #[test]
    fn fingerprint_session_seed_is_random_and_fixed_width() {
        let first = new_fingerprint_seed().expect("system random source should be available");
        let second = new_fingerprint_seed().expect("system random source should be available");

        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
    }

    #[test]
    fn filter_list_parser_keeps_safe_domain_rules_and_exceptions() {
        let lists = parse_filter_list_domains(
            r#"
                ! comment
                [Adblock Plus 2.0]
                ||ads.example.com^
                ||TRACKER.example.net^$script,third-party
                @@||allowed.example.org^
                ||site.example.com/banner
                ||scoped.example.com^$domain=publisher.example
                ||first-party.example.com^$~third-party
                example.com##.ad
            "#,
        );

        assert_eq!(
            lists.blocked.into_iter().collect::<HashSet<_>>(),
            HashSet::from([
                "ads.example.com".to_string(),
                "tracker.example.net".to_string(),
            ])
        );
        assert_eq!(lists.allowed, vec!["allowed.example.org".to_string()]);
    }

    #[test]
    fn downloaded_domain_rules_and_exceptions_are_emitted_for_webkit() {
        let lists = FilterListDomains {
            blocked: vec!["ads.example.com".into()],
            allowed: vec!["allowed.example.com".into()],
        };
        let rules: serde_json::Value = serde_json::from_str(&filter_rules_json_with_lists(&lists))
            .expect("generated list rules must be valid JSON");
        let rules = rules.as_array().expect("filter rules must be an array");

        assert!(rules.iter().any(|rule| {
            rule["trigger"]["url-filter"]
                .as_str()
                .is_some_and(|pattern| pattern.contains("ads\\.example\\.com"))
                && rule["action"]["type"] == "block"
        }));
        assert!(rules.iter().any(|rule| {
            rule["trigger"]["url-filter"]
                .as_str()
                .is_some_and(|pattern| pattern.contains("allowed\\.example\\.com"))
                && rule["action"]["type"] == "ignore-previous-rules"
        }));
    }

    #[test]
    fn shields_rules_cover_ad_domains_resources_and_cosmetic_selectors() {
        let rules: serde_json::Value =
            serde_json::from_str(&filter_rules_json_with_lists(&FilterListDomains::default()))
                .expect("filter rules must be valid JSON");
        let rules = rules.as_array().expect("filter rules must be an array");

        assert_eq!(rules.len(), TRACKER_DOMAINS.len() + 2);
        assert!(TRACKER_DOMAINS.contains(&"doubleclick.net"));
        assert!(TRACKER_DOMAINS.contains(&"adzerk.net"));
        assert!(rules.iter().any(|rule| {
            rule["action"]["type"] == "css-display-none"
                && rule["action"]["selector"]
                    .as_str()
                    .is_some_and(|selector| selector.contains("ytd-ad-slot-renderer"))
                && !rule["action"]["selector"]
                    .as_str()
                    .is_some_and(|selector| selector.contains(".ytp-ad-module"))
                && rule["trigger"]["if-domain"][0] == "youtube.com"
        }));
        assert!(rules.iter().any(|rule| {
            rule["action"]["type"] == "css-display-none"
                && rule["trigger"]["if-domain"][0] == "duckduckgo.com"
        }));
        assert!(rules.iter().all(|rule| {
            rule["action"]["type"] != "css-display-none"
                || rule["trigger"].get("if-domain").is_some()
        }));
        assert!(
            TRACKER_DOMAINS
                .iter()
                .all(|domain| !domain.trim().is_empty() && *domain == domain.trim())
        );
        assert_eq!(
            TRACKER_DOMAINS.iter().collect::<HashSet<_>>().len(),
            TRACKER_DOMAINS.len(),
            "duplicate domains generate duplicate filter rules"
        );
    }

    #[test]
    fn ad_cleaner_hides_common_ad_placements_and_skips_youtube_video_ads() {
        assert!(AD_CLEANER_SCRIPT.contains("[data-ad-slot]"));
        assert!(AD_CLEANER_SCRIPT.contains("ytd-ad-slot-renderer"));
        assert!(AD_CLEANER_SCRIPT.contains("iframe[src*=\"doubleclick.net\"]"));
        assert!(AD_CLEANER_SCRIPT.contains(".ytp-ad-skip-button"));
        assert!(AD_CLEANER_SCRIPT.contains("video.currentTime = video.duration"));
        assert!(AD_CLEANER_SCRIPT.contains(".html5-video-player.ad-showing"));
    }

    #[test]
    fn search_engine_preferences_map_to_known_search_endpoints() {
        assert_eq!(search_url("duckduckgo"), "https://duckduckgo.com/?q=");
        assert_eq!(search_url("google"), "https://www.google.com/search?q=");
        assert_eq!(search_url("brave"), "https://search.brave.com/search?q=");
        assert_eq!(search_url("unknown"), search_url("duckduckgo"));
    }

    #[test]
    fn full_http_urls_are_loaded_without_becoming_searches() {
        let url = "https://www.youtube.com/watch?v=video123&list=mix";
        assert_eq!(normalize_input(url, "duckduckgo"), url);
        assert_eq!(
            normalize_input("HTTPS://www.youtube.com/watch?v=video123", "duckduckgo"),
            "https://www.youtube.com/watch?v=video123"
        );
        assert_eq!(
            normalize_input("https://example.com/a page?q=hello world", "google"),
            "https://example.com/a%20page?q=hello%20world"
        );
        assert_eq!(
            normalize_input("http://example.com/", "brave"),
            "http://example.com/"
        );
    }

    #[test]
    fn bare_hosts_and_search_terms_are_distinguished() {
        assert_eq!(
            normalize_input("www.youtube.com/watch?v=video123", "duckduckgo"),
            "https://www.youtube.com/watch?v=video123"
        );
        assert_eq!(
            normalize_input("cats and dogs", "google"),
            "https://www.google.com/search?q=cats%20and%20dogs"
        );
    }
}
