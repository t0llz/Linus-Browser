use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Entry, FileFilter, Label,
    Notebook, Orientation, Revealer, RevealerTransitionType, ToggleButton, gio, glib,
};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};
use webkit6::prelude::*;
use webkit6::*;

static NEXT_WALLPAPER_ID: AtomicU64 = AtomicU64::new(0);

const HOME_URL: &str = "about:home";
const PRIVACY_URL: &str = "about:privacy";
const HOME_URI: &str = "linus://home";
const PRIVACY_URI: &str = "linus://privacy";
const HOME_HTML: &str = r##"<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>New Tab</title>
<style>
*{box-sizing:border-box}html,body{width:100%;height:100%;margin:0;overflow:hidden}
body{background:#17191d;color:#f7f7f8;font:14px Cantarell,Inter,Arial,sans-serif}
.page{position:relative;display:grid;place-items:center;width:100%;height:100%;min-height:360px;isolation:isolate;background:url('{{WALLPAPER_DATA}}') center 53%/cover no-repeat}
.page:before{content:"";position:absolute;inset:0;z-index:-1;background:radial-gradient(ellipse at 50% 44%,#11131808 0%,#11131835 68%,#111318a8 100%),linear-gradient(180deg,#11131855 0%,#11131808 34%,#11131858 100%)}
.hero{width:min(620px,calc(100vw - 36px));display:flex;flex-direction:column;align-items:center;margin-top:-2vh;text-align:center;text-shadow:0 2px 18px #1119}
.brand{width:76px;height:76px;object-fit:contain;filter:drop-shadow(0 8px 18px #1118)}
.clock{margin-top:16px;font-size:clamp(46px,7vw,76px);font-weight:650;letter-spacing:-3.5px;line-height:1}
.greeting{margin:10px 0 27px;color:#fff;font-size:16px;font-weight:500;letter-spacing:.1px}
.search{width:min(540px,100%);height:56px;display:flex;align-items:center;gap:13px;padding:0 17px;background:#202124e8;border:1px solid #ffffff30;border-radius:18px;box-shadow:0 14px 38px #1116;backdrop-filter:blur(18px);transition:border-color .15s,background .15s,transform .15s,box-shadow .15s}
.search:focus-within{border-color:#ff684d;box-shadow:0 0 0 3px #ff684d35,0 16px 42px #1118;transform:translateY(-1px)}
.search img{width:22px;height:24px;object-fit:contain;flex:none}
.search input{flex:1;min-width:0;background:transparent;border:0;outline:0;color:#f7f7f8;font:inherit;font-size:15px;text-shadow:none}
.search input::placeholder{color:#b4b5ba}
.search button{width:34px;height:34px;display:grid;place-items:center;padding:0;border:0;border-radius:10px;background:transparent;color:#b4b5ba;cursor:pointer}
.search button:hover{background:#ffffff15;color:#fff}.search button svg{width:19px;height:19px}
.shortcuts{display:flex;justify-content:center;flex-wrap:wrap;gap:14px;margin-top:27px}
.shortcut{display:flex;flex-direction:column;align-items:center;gap:9px;width:78px;color:#fff;text-decoration:none;font-size:12px;text-shadow:0 1px 8px #111}
.shortcut-icon{width:48px;height:48px;display:grid;place-items:center;border:1px solid #ffffff35;border-radius:16px;background:#202124c9;backdrop-filter:blur(12px);font-size:17px;font-weight:600;box-shadow:0 8px 24px #1114;transition:transform .15s,border-color .15s,background .15s}
.shortcut:hover .shortcut-icon{transform:translateY(-2px);border-color:#ffffff66;background:#34363be8}
.shortcut:focus-visible,.settings:focus-visible,.search button:focus-visible{outline:2px solid #ff684d;outline-offset:3px}
.settings{position:absolute;top:22px;right:24px;width:42px;height:42px;display:grid;place-items:center;border:1px solid #ffffff30;border-radius:14px;background:#202124bd;color:#fff;text-decoration:none;backdrop-filter:blur(14px);transition:border-color .15s,background .15s}
.settings:hover{background:#34363bd9}.settings svg{width:20px;height:20px;fill:none;stroke:currentColor;stroke-width:1.7;stroke-linecap:round;stroke-linejoin:round}
.light .page:before{background:radial-gradient(ellipse at 50% 44%,#fff0 0%,#11131818 70%,#11131870 100%),linear-gradient(180deg,#fff3 0%,#fff0 40%,#11131845 100%)}
.light .search{background:#ffffffed;border-color:#ffffffa8;box-shadow:0 12px 36px #27303a30}
.light .search input{color:#202124;text-shadow:none}.light .search input::placeholder{color:#666b73}
.light .search button{color:#62666d}.light .search button:hover{background:#20212410;color:#202124}
.light .shortcut{color:#fff}.light .shortcut-icon,.light .settings{background:#ffffffdf;border-color:#ffffffc8;color:#202124}
.light .shortcut-icon{box-shadow:0 6px 20px #20212420}.light .shortcut:hover .shortcut-icon{background:#fff}
@media(max-height:560px){.hero{margin-top:0}.brand{width:56px;height:56px}.clock{margin-top:8px;font-size:48px}.greeting{margin:6px 0 14px}.search{height:48px}.shortcuts{margin-top:16px}}
@media(prefers-reduced-motion:reduce){*,*::before,*::after{transition:none!important;scroll-behavior:auto!important}}
</style></head>
<body class="{{THEME_CLASS}}"><main class="page">
<a class="settings" href="about:privacy" aria-label="Privacy and settings" title="Privacy and settings"><svg viewBox="0 0 24 24"><path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z"/><path d="m19.4 15 .1.1a1.8 1.8 0 1 1-2.5 2.5l-.1-.1a1.8 1.8 0 0 0-3 .9v.2a1.8 1.8 0 1 1-3.6 0v-.2a1.8 1.8 0 0 0-3-.9l-.1.1a1.8 1.8 0 1 1-2.5-2.5l.1-.1a1.8 1.8 0 0 0-.9-3H3.7a1.8 1.8 0 1 1 0-3.6h.2a1.8 1.8 0 0 0 .9-3l-.1-.1a1.8 1.8 0 1 1 2.5-2.5l.1.1a1.8 1.8 0 0 0 3-.9v-.2a1.8 1.8 0 1 1 3.6 0v.2a1.8 1.8 0 0 0 3 .9l.1-.1a1.8 1.8 0 1 1 2.5 2.5l-.1.1a1.8 1.8 0 0 0 .9 3h.2a1.8 1.8 0 1 1 0 3.6h-.2a1.8 1.8 0 0 0-.9 3Z"/></svg></a>
<section class="hero">
<img class="brand" src="{{TUX_DATA}}" alt="Linus Browser">
<div class="clock" id="clock" aria-label="Current time"></div>
<p class="greeting" id="greeting">A little more privacy. A lot more web.</p>
<form class="search" onsubmit="event.preventDefault();location.href='{{SEARCH_PREFIX}}'+encodeURIComponent(this.q.value)">
<img src="{{TUX_DATA}}" alt="">
<input name="q" autofocus aria-label="Search the web" placeholder="Search the web or enter an address">
<button type="submit" aria-label="Search"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="10.8" cy="10.8" r="6.8"/><path d="m16 16 5 5"/></svg></button>
</form>
<nav class="shortcuts" aria-label="Quick links">
<a class="shortcut" href="https://duckduckgo.com"><span class="shortcut-icon">D</span><span>DuckDuckGo</span></a>
<a class="shortcut" href="https://www.wikipedia.org"><span class="shortcut-icon">W</span><span>Wikipedia</span></a>
<a class="shortcut" href="https://github.com"><span class="shortcut-icon">⌘</span><span>GitHub</span></a>
</nav>
</section>
<script>
const clock=document.getElementById('clock');
function updateClock(){clock.textContent=new Intl.DateTimeFormat(undefined,{hour:'numeric',minute:'2-digit'}).format(new Date())}
updateClock();setInterval(updateClock,15000);
</script>
</main></body></html>"##;

const PRIVACY_HTML: &str = r##"<!doctype html><meta charset="utf-8"><style>*{box-sizing:border-box}body{background:#191a1e;color:#eee;font:16px Cantarell,Inter,sans-serif;max-width:760px;margin:80px auto;padding:24px}h1{color:#ff563b;font-size:34px}p{line-height:1.7;color:#c7c5ca}.panel{padding:18px;background:#25262b;border:1px solid #37383f;border-radius:12px;margin:14px 0}.light body{background:#f5f6f8;color:#202124}.light p{color:#50545a}.light .panel{background:#fff;border-color:#d9dce1}</style><body class="{{THEME_CLASS}}"><h1>Privacy controls</h1><p>Linus Browser uses an ephemeral WebKit data store. Cookies and site storage are kept in memory and discarded when the browser closes.</p><div class="panel"><b>Third-party cookies</b><p>Blocked while Shields are enabled. Toggle the shield button to allow them for this browser session.</p></div><div class="panel"><b>Scripts and page content</b><p>JavaScript and site resources are enabled so modern websites can work. You can disable scripts from the privacy menu.</p></div><div class="panel"><b>Downloads</b><p>Downloads are saved to your Downloads folder.</p></div></body>"##;

const CSS_DARK: &str = r#"
window, .browser-root { background: #1e1e24; color: #f1f1f1; }
.browser-notebook { background: #1e1e24; border: none; }
.browser-notebook > header { background: #151518; border-bottom: 1px solid #2b2b36; padding: 5px 8px 0; }
.browser-notebook > header > tabs > tab { background: #1e1e24; color: #9a9ab0; border-radius: 9px 9px 0 0; padding: 7px 12px; border: none; margin-right: 3px; }
.browser-notebook > header > tabs > tab:checked { background: #2b2b36; color: #fff; border-bottom: 2px solid #ff5000; }
.tab-shell { background: transparent; min-width: 180px; }
.tab-item { background: transparent; border: 0; box-shadow: none; color: #f1f1f1; padding: 5px 4px 5px 6px; }
.tab-logo-badge { background: #f2f0eb; border: 1px solid #ffffff30; border-radius: 50%; min-width: 22px; min-height: 22px; }
.tab-logo-badge image { min-width: 15px; min-height: 15px; }
.tab-close, .new-tab { background: transparent; border: 0; box-shadow: none; color: #f1f1f1; padding: 4px 7px; border-radius: 7px; }
.tab-close { min-width: 28px; min-height: 30px; }
.new-tab { min-width: 36px; min-height: 34px; font-size: 19px; }
.new-tab:hover, .tab-close:hover { background: #3a3a4c; color: #fff; }
.browser-toolbar { background: #19191d; color: #f1f1f1; border-top: 1px solid #2b2b36; padding: 8px 12px; }
.nav-button, .action-button, .engine-button { background: #2b2b36; border: 1px solid transparent; box-shadow: none; color: #fff; border-radius: 11px; padding: 5px 9px; min-width: 48px; min-height: 48px; transition: background 140ms ease, border-color 140ms ease; }
.nav-button image { min-width: 18px; min-height: 18px; }
.nav-button:hover, .action-button:hover { background: #3a3a4c; color: #fff; }
.nav-button:focus-visible, .action-button:focus-visible, .new-tab:focus-visible { outline: 2px solid #ff5000; outline-offset: 1px; }
.nav-button:active, .action-button:active { background: #ff5000; color: #fff; }
.engine-button { background: #2b2b36; border: 1px solid #3a3a4c; padding: 4px 8px; }
.engine-button:hover { background: #3a3a4c; }
.load-progress { min-height: 2px; }
.load-progress trough { min-height: 2px; background: transparent; }
.load-progress progress { min-height: 2px; background: #ff5000; }
.address-shell { background: #25252e; border: 1px solid #3a3a4c; border-radius: 20px; min-height: 48px; }
.address-shell:focus-within { border: 1px solid #ff5000; box-shadow: 0 0 0 2px #ff500033; }
.address-entry { background: transparent; color: #fff; border: 0; box-shadow: none; border-radius: 20px; padding: 8px 14px; min-height: 44px; font-size: 14px; }
.address-entry:focus { background: transparent; border: 0; box-shadow: none; }
.address-entry image { color: #f1f1f1; }
.address-shell .action-button { min-width: 34px; min-height: 32px; margin: 2px; padding: 4px; }
.bookmark-bar { background: #19191d; padding: 5px 12px; }
.bookmark-item { background: transparent; border: 0; color: #f1f1f1; padding: 5px 9px; border-radius: 8px; }
.bookmark-item:hover { background: #3a3a4c; }
.popover { background: #2b2b36; color: #fff; border: 1px solid #3a3a4c; }
.engine-choice { background: transparent; border: 0; color: #fff; padding: 8px 14px; }
.engine-choice:hover { background: #3a3a4c; }
.menu-heading { color: #aeb0b6; padding: 4px 8px; font-size: 11px; font-weight: 700; }
.status { background: #19191d; color: #aeb0b6; padding: 3px 8px; font-size: 11px; }
scrollbar slider { min-width: 6px; min-height: 6px; border-radius: 3px; background: #4a4a5a; }
scrollbar slider:hover { background: #ff5000; }
"#;

const CSS_LIGHT: &str = r#"
window, .browser-root, .browser-notebook { background: #f3f4f7; color: #202124; }
.browser-notebook > header { background: #e7e8ed; border-bottom-color: #d5d7df; }
.browser-notebook > header > tabs > tab { background: #e7e8ed; color: #60636d; }
.browser-notebook > header > tabs > tab:checked { background: #fff; color: #202124; border-bottom-color: #ff5000; }
.tab-item { color: #5f6368; }
.tab-logo-badge { background: #f2f0eb; border: 1px solid #d5d7da; border-radius: 50%; min-width: 22px; min-height: 22px; }
.tab-close, .new-tab { color: #5f6368; }
.new-tab:hover, .tab-close:hover { background: #d4d6da; color: #202124; }
.browser-toolbar { background: #e7e8ed; color: #343640; border-top-color: #d5d7df; }
.nav-button, .action-button, .engine-button { color: #5f6368; }
.nav-button image { min-width: 16px; min-height: 16px; }
.browser-toolbar .nav-button, .browser-toolbar .action-button { background: #d9dbe2; color: #393b45; }
.nav-button:hover, .action-button:hover { background: #cdd0d9; color: #202124; }
.nav-button:active, .action-button:active { background: #ff5000; color: #fff; }
.engine-button { background: #fff; border-color: #dadce0; }
.engine-button:hover { background: #e9eaed; }
.address-shell { background: #fff; border-color: #cdd0d8; }
.address-shell:focus-within { border-color: #ff5000; }
.address-entry { background: transparent; color: #202124; }
.address-entry image { color: #5f6368; }
.bookmark-bar { background: #f1f3f4; }
.bookmark-item { color: #414449; }
.bookmark-item:hover { background: #e2e4e8; }
.popover { background: #fff; color: #202124; border: 1px solid #dadce0; }
.engine-choice { color: #202124; }
.engine-choice:hover { background: #f1f3f4; }
.menu-heading { color: #5f6368; }
.status { background: #e7e8ed; color: #656873; }
"#;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    title: String,
    url: String,
}

fn address_text(uri: &str) -> &str {
    match uri {
        HOME_URI | HOME_URL => "",
        _ => display_uri(uri),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SearchEngine {
    #[default]
    DuckDuckGo,
    Google,
    Wikipedia,
}

#[test]
fn new_tab_address_bar_hides_internal_uri() {
    assert_eq!(address_text(HOME_URL), "");
    assert_eq!(address_text(HOME_URI), "");
    assert_eq!(address_text(PRIVACY_URI), PRIVACY_URL);
    assert_eq!(address_text("https://example.com"), "https://example.com");
}

impl SearchEngine {
    fn label(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Google => "Google",
            Self::Wikipedia => "Wikipedia",
        }
    }

    fn search_url(self, query: &str) -> String {
        let query = percent_encode(query);
        match self {
            Self::DuckDuckGo => format!("https://duckduckgo.com/?q={query}"),
            Self::Google => format!("https://www.google.com/search?q={query}"),
            Self::Wikipedia => format!("https://en.wikipedia.org/w/index.php?search={query}"),
        }
    }

    fn search_prefix(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "https://duckduckgo.com/?q=",
            Self::Google => "https://www.google.com/search?q=",
            Self::Wikipedia => "https://en.wikipedia.org/w/index.php?search=",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
enum AppearanceMode {
    #[default]
    Dark,
    System,
    Light,
}

impl AppearanceMode {
    fn label(self) -> &'static str {
        match self {
            Self::System => "Use system theme",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    fn is_dark(self) -> bool {
        match self {
            Self::System => system_prefers_dark(),
            Self::Dark => true,
            Self::Light => false,
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
struct BrowserPreferences {
    #[serde(default)]
    appearance: AppearanceMode,
    #[serde(default)]
    wallpaper: Option<PathBuf>,
}

struct NewTabWallpaper {
    path: Option<PathBuf>,
    data_url: String,
}

fn system_prefers_dark() -> bool {
    gtk::Settings::default()
        .and_then(|settings| settings.gtk_theme_name())
        .is_some_and(|theme| theme.to_ascii_lowercase().contains("dark"))
}

struct BrowserTab {
    id: u64,
    view: WebView,
    tab_container: GtkBox,
    tab_title: Label,
    title: String,
    uri: String,
}

struct Browser {
    window: ApplicationWindow,
    notebook: Notebook,
    new_tab_button: Button,
    address: Entry,
    back: Button,
    forward: Button,
    reload: Button,
    bookmark: Button,
    shield: ToggleButton,
    status: Label,
    progress: gtk::ProgressBar,
    css_provider: CssProvider,
    bookmark_bar: GtkBox,
    bookmark_revealer: Revealer,
    tabs: RefCell<Vec<BrowserTab>>,
    active_tab: Cell<u64>,
    next_tab_id: Cell<u64>,
    context: WebContext,
    network_session: NetworkSession,
    bookmarks: RefCell<Vec<Bookmark>>,
    bookmarks_path: PathBuf,
    scripts_enabled: Cell<bool>,
    search_engine: Rc<Cell<SearchEngine>>,
    appearance: Rc<Cell<AppearanceMode>>,
    new_tab_wallpaper: Rc<RefCell<NewTabWallpaper>>,
    preferences_path: PathBuf,
}

pub fn connect(application: &Application) {
    let browser: Rc<RefCell<Option<Rc<Browser>>>> = Rc::new(RefCell::new(None));
    let browser_slot = browser.clone();
    application.connect_activate(move |application| {
        let mut browser = browser_slot.borrow_mut();
        if let Some(browser) = browser.as_ref() {
            browser.window.present();
        } else {
            *browser = Some(build_browser(application));
        }
    });
}

fn build_browser(application: &Application) -> Rc<Browser> {
    let preferences_path = preferences_path();
    let (preferences, preferences_error) = match load_preferences(&preferences_path) {
        Ok(preferences) => (preferences, None),
        Err(error) => (
            BrowserPreferences {
                appearance: AppearanceMode::System,
                wallpaper: None,
            },
            Some(error),
        ),
    };
    let (wallpaper, wallpaper_error) =
        match load_new_tab_wallpaper(preferences.wallpaper.as_deref()) {
            Ok(wallpaper) => (wallpaper, None),
            Err(error) => (default_new_tab_wallpaper(), Some(error)),
        };
    let new_tab_wallpaper = Rc::new(RefCell::new(wallpaper));
    let appearance = Rc::new(Cell::new(preferences.appearance));
    let (css_provider, css_error) = install_css(preferences.appearance);
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(preferences.appearance.is_dark());
    }

    let context = WebContext::new();
    let network_session = NetworkSession::new_ephemeral();
    let search_engine = Rc::new(Cell::new(SearchEngine::default()));
    register_internal_scheme(
        &context,
        search_engine.clone(),
        appearance.clone(),
        new_tab_wallpaper.clone(),
    );
    if let Some(cookie_manager) = network_session.cookie_manager() {
        cookie_manager.set_accept_policy(CookieAcceptPolicy::NoThirdParty);
    }

    let window = ApplicationWindow::new(application);
    window.set_title(Some("Linus Browser"));
    window.set_default_size(1200, 780);

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("browser-root");

    let notebook = Notebook::new();
    notebook.add_css_class("browser-notebook");
    notebook.set_show_border(false);
    notebook.set_scrollable(true);
    notebook.set_hexpand(true);
    notebook.set_vexpand(true);
    let new_tab = Button::with_label("+");
    new_tab.set_tooltip_text(Some("New tab (Ctrl+T)"));
    new_tab.set_size_request(34, -1);
    new_tab.add_css_class("new-tab");
    notebook.set_action_widget(&new_tab, gtk::PackType::End);
    let toolbar = GtkBox::new(Orientation::Horizontal, 8);
    toolbar.add_css_class("browser-toolbar");
    let back = icon_button("go-previous-symbolic", "Back");
    let forward = icon_button("go-next-symbolic", "Forward");
    let reload = icon_button("view-refresh-symbolic", "Reload");
    let navigation_controls = GtkBox::new(Orientation::Horizontal, 4);
    navigation_controls.add_css_class("nav-controls");
    navigation_controls.append(&back);
    navigation_controls.append(&forward);
    navigation_controls.append(&reload);
    let address = Entry::new();
    address.set_placeholder_text(Some("Search or type a URL"));
    address.set_hexpand(true);
    address.set_icon_from_icon_name(
        gtk::EntryIconPosition::Primary,
        Some("system-search-symbolic"),
    );
    address.set_icon_tooltip_text(
        gtk::EntryIconPosition::Primary,
        Some("Choose search engine"),
    );
    address.add_css_class("address-entry");
    let bookmark = icon_button("bookmark-new-symbolic", "Bookmark this page");
    let shield = ToggleButton::new();
    let shield_image = gtk::Image::from_icon_name("security-high-symbolic");
    shield_image.set_pixel_size(16);
    shield.set_child(Some(&shield_image));
    shield.set_active(true);
    shield.set_tooltip_text(Some("Privacy shield: block third-party cookies"));
    shield.add_css_class("action-button");
    let downloads = icon_button("folder-download-symbolic", "Download current page");
    let menu = icon_button("open-menu-symbolic", "Bookmarks and privacy settings");

    toolbar.append(&navigation_controls);
    let address_shell = GtkBox::new(Orientation::Horizontal, 0);
    address_shell.add_css_class("address-shell");
    address_shell.set_hexpand(true);
    address_shell.append(&address);
    address_shell.append(&shield);
    toolbar.append(&address_shell);
    toolbar.append(&bookmark);
    toolbar.append(&downloads);
    toolbar.append(&menu);

    let bookmark_revealer = Revealer::new();
    bookmark_revealer.set_transition_type(RevealerTransitionType::SlideDown);
    bookmark_revealer.set_reveal_child(false);
    let bookmark_bar = GtkBox::new(Orientation::Horizontal, 3);
    bookmark_bar.add_css_class("bookmark-bar");
    bookmark_revealer.set_child(Some(&bookmark_bar));

    let status = Label::new(Some("Private browsing · WebKit engine"));
    status.set_xalign(0.0);
    status.add_css_class("status");

    let progress = gtk::ProgressBar::new();
    progress.add_css_class("load-progress");
    progress.set_size_request(-1, 2);
    progress.set_visible(false);
    root.append(&notebook);
    root.append(&toolbar);
    root.append(&bookmark_revealer);
    root.append(&progress);
    root.append(&status);
    window.set_child(Some(&root));

    let (bookmarks, bookmark_error) = match load_bookmarks(&bookmarks_path()) {
        Ok(bookmarks) => (bookmarks, None),
        Err(error) => (Vec::new(), Some(error)),
    };

    let browser = Rc::new(Browser {
        window,
        notebook,
        new_tab_button: new_tab.clone(),
        address,
        back,
        forward,
        reload,
        bookmark,
        shield,
        status,
        bookmark_bar,
        bookmark_revealer,
        progress,
        css_provider,
        tabs: RefCell::new(Vec::new()),
        active_tab: Cell::new(0),
        next_tab_id: Cell::new(1),
        context,
        network_session,
        bookmarks: RefCell::new(bookmarks),
        bookmarks_path: bookmarks_path(),
        scripts_enabled: Cell::new(true),
        search_engine,
        appearance,
        new_tab_wallpaper,
        preferences_path,
    });

    if let Some(error) = bookmark_error {
        browser
            .status
            .set_text(&format!("Could not read bookmarks: {error}"));
    }
    if let Some(error) = preferences_error {
        browser
            .status
            .set_text(&format!("Could not read browser settings: {error}"));
    }
    if let Some(error) = wallpaper_error {
        browser
            .status
            .set_text(&format!("Could not load new-tab background: {error}"));
    }
    if let Some(error) = css_error {
        browser
            .status
            .set_text(&format!("Could not apply browser theme: {error}"));
    }

    connect_toolbar(&browser, &downloads, &menu);
    connect_notebook(&browser);
    connect_system_theme(&browser);
    connect_downloads(&browser);
    rebuild_bookmark_bar(&browser);
    add_tab(&browser, None);
    browser.window.present();
    browser.bookmark_revealer.set_reveal_child(false);
    browser
}

fn install_css(mode: AppearanceMode) -> (CssProvider, Option<String>) {
    let provider = CssProvider::new();
    let css = browser_css(mode.is_dark());
    let error = load_css(&provider, &css).err();
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    (provider, error)
}

fn load_css(provider: &CssProvider, css: &str) -> Result<(), String> {
    let parse_error = Rc::new(RefCell::new(None));
    let parse_error_slot = parse_error.clone();
    let handler = provider.connect_parsing_error(move |_, _, error| {
        *parse_error_slot.borrow_mut() = Some(error.to_string());
    });
    provider.load_from_data(css);
    provider.disconnect(handler);

    match parse_error.borrow_mut().take() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn browser_css(dark: bool) -> String {
    if dark {
        CSS_DARK.to_string()
    } else {
        format!("{CSS_DARK}\n{CSS_LIGHT}")
    }
}

fn register_internal_scheme(
    context: &WebContext,
    search_engine: Rc<Cell<SearchEngine>>,
    appearance: Rc<Cell<AppearanceMode>>,
    wallpaper: Rc<RefCell<NewTabWallpaper>>,
) {
    let active_engine = search_engine.clone();
    context.register_uri_scheme("linus", move |request| {
        let html = match request.uri().as_deref().unwrap_or_default() {
            HOME_URI => home_html(
                active_engine.get(),
                appearance.get().is_dark(),
                &wallpaper.borrow().data_url,
            ),
            PRIVACY_URI => privacy_html(appearance.get().is_dark()),
            _ => "<!doctype html><title>Not found</title><h1>Page not found</h1>".into(),
        };
        let bytes = glib::Bytes::from_owned(html.into_bytes());
        let stream = gio::MemoryInputStream::from_bytes(&bytes);
        request.finish(&stream, bytes.len() as i64, Some("text/html"));
    });
}

fn connect_toolbar(browser: &Rc<Browser>, downloads: &Button, menu: &Button) {
    let weak = Rc::downgrade(browser);
    browser.address.connect_activate(move |entry| {
        if let Some(browser) = weak.upgrade() {
            let target = normalize_address(&entry.text(), browser.search_engine.get());
            navigate_active(&browser, &target);
        }
    });
    let weak = Rc::downgrade(browser);
    browser.address.connect_icon_press(move |entry, position| {
        if position != gtk::EntryIconPosition::Primary {
            return;
        }
        if let Some(browser) = weak.upgrade() {
            show_search_engine_menu(&browser, entry);
        }
    });

    let weak = Rc::downgrade(browser);
    browser.back.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade()
            && let Some(tab) = active_tab(&browser)
            && tab.view.can_go_back()
        {
            tab.view.go_back();
        }
    });
    let weak = Rc::downgrade(browser);
    browser.forward.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade()
            && let Some(tab) = active_tab(&browser)
            && tab.view.can_go_forward()
        {
            tab.view.go_forward();
        }
    });
    let weak = Rc::downgrade(browser);
    browser.reload.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade()
            && let Some(tab) = active_tab(&browser)
        {
            if tab.view.is_loading() {
                tab.view.stop_loading();
            } else {
                tab.view.reload();
            }
        }
    });
    let weak = Rc::downgrade(browser);
    browser.bookmark.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade() {
            toggle_bookmark(&browser);
        }
    });
    let weak = Rc::downgrade(browser);
    browser.shield.connect_toggled(move |button| {
        if let Some(browser) = weak.upgrade() {
            set_shields(&browser, button.is_active());
        }
    });
    let weak = Rc::downgrade(browser);
    browser.new_tab_button.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade() {
            add_tab(&browser, None);
        }
    });
    let weak = Rc::downgrade(browser);
    let key_controller = gtk::EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, keyval, _, state| {
        if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) && keyval == gtk::gdk::Key::t {
            if let Some(browser) = weak.upgrade() {
                add_tab(&browser, None);
            }
            return glib::Propagation::Stop;
        }
        if state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            && keyval == gtk::gdk::Key::l
            && let Some(browser) = weak.upgrade()
        {
            browser.address.grab_focus();
            browser.address.select_region(0, -1);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    browser.window.add_controller(key_controller);
    let weak = Rc::downgrade(browser);
    downloads.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade()
            && let Some(tab) = active_tab(&browser)
        {
            let uri = tab.view.uri().map(|value| value.to_string());
            if let Some(uri) =
                uri.filter(|uri| uri.starts_with("http://") || uri.starts_with("https://"))
            {
                tab.view.download_uri(&uri);
                browser.status.set_text("Starting download…");
            } else {
                browser
                    .status
                    .set_text("Open a web page before downloading.");
            }
        }
    });

    let weak = Rc::downgrade(browser);
    menu.connect_clicked(move |button| {
        if let Some(browser) = weak.upgrade() {
            show_browser_menu(&browser, button);
        }
    });
}

fn connect_system_theme(browser: &Rc<Browser>) {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    let weak = Rc::downgrade(browser);
    settings.connect_gtk_theme_name_notify(move |_| {
        if let Some(browser) = weak.upgrade()
            && browser.appearance.get() == AppearanceMode::System
        {
            apply_appearance(&browser);
        }
    });
}

fn connect_notebook(browser: &Rc<Browser>) {
    let weak = Rc::downgrade(browser);
    browser.notebook.connect_switch_page(move |_, page, _| {
        let Some(view) = page.downcast_ref::<WebView>() else {
            return;
        };
        if let Some(browser) = weak.upgrade()
            && let Some(id) = browser
                .tabs
                .borrow()
                .iter()
                .find(|tab| tab.view == *view)
                .map(|tab| tab.id)
            && browser.active_tab.get() != id
        {
            activate_tab(&browser, id);
        }
    });
}

fn set_appearance(browser: &Rc<Browser>, appearance: AppearanceMode) {
    browser.appearance.set(appearance);
    apply_appearance(browser);
    let wallpaper = browser.new_tab_wallpaper.borrow().path.clone();
    if let Err(error) = save_browser_preferences(browser, wallpaper) {
        browser
            .status
            .set_text(&format!("Could not save appearance setting: {error}"));
    }
}

fn save_browser_preferences(browser: &Browser, wallpaper: Option<PathBuf>) -> Result<(), String> {
    save_preferences(
        &browser.preferences_path,
        &BrowserPreferences {
            appearance: browser.appearance.get(),
            wallpaper,
        },
    )
}

fn set_new_tab_wallpaper(browser: &Rc<Browser>, source: &Path) -> Result<(), String> {
    let (path, data_url) = persist_new_tab_wallpaper(source)?;
    if let Err(error) = save_browser_preferences(browser, Some(path.clone())) {
        return match fs::remove_file(&path) {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(format!(
                "{error}; could not remove temporary background {}: {cleanup_error}",
                path.display()
            )),
        };
    }
    *browser.new_tab_wallpaper.borrow_mut() = NewTabWallpaper {
        path: Some(path),
        data_url,
    };
    reload_home_tabs(browser);
    browser.status.set_text("New-tab background updated.");
    Ok(())
}

fn reset_new_tab_wallpaper(browser: &Rc<Browser>) -> Result<(), String> {
    save_browser_preferences(browser, None)?;
    *browser.new_tab_wallpaper.borrow_mut() = default_new_tab_wallpaper();
    reload_home_tabs(browser);
    browser
        .status
        .set_text("Default new-tab background restored.");
    Ok(())
}

fn reload_home_tabs(browser: &Browser) {
    for tab in browser.tabs.borrow().iter() {
        if tab.uri == HOME_URL {
            tab.view.load_uri(HOME_URI);
        }
    }
}

fn apply_appearance(browser: &Rc<Browser>) {
    let appearance = browser.appearance.get();
    let css = browser_css(appearance.is_dark());
    if let Err(error) = load_css(&browser.css_provider, &css) {
        browser
            .status
            .set_text(&format!("Could not apply browser theme: {error}"));
        return;
    }
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(appearance.is_dark());
    }
    for tab in browser.tabs.borrow().iter() {
        if tab.uri == HOME_URL {
            tab.view.load_uri(HOME_URI);
        }
    }
    browser
        .status
        .set_text(&format!("Appearance: {}", appearance.label()));
}

fn show_search_engine_menu(browser: &Rc<Browser>, relative_to: &impl IsA<gtk::Widget>) {
    let popover = gtk::Popover::new();
    popover.set_parent(relative_to);
    popover.add_css_class("popover");
    let content = GtkBox::new(Orientation::Vertical, 4);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(8);
    content.set_margin_end(8);
    for engine in [
        SearchEngine::DuckDuckGo,
        SearchEngine::Google,
        SearchEngine::Wikipedia,
    ] {
        let label = if browser.search_engine.get() == engine {
            format!("✓  {}", engine.label())
        } else {
            format!("     {}", engine.label())
        };
        let choice = Button::with_label(&label);
        choice.set_halign(gtk::Align::Fill);
        choice.add_css_class("engine-choice");
        let weak = Rc::downgrade(browser);
        let close_menu = popover.clone();
        choice.connect_clicked(move |_| {
            if let Some(browser) = weak.upgrade() {
                browser.search_engine.set(engine);
                browser.address.set_icon_tooltip_text(
                    gtk::EntryIconPosition::Primary,
                    Some(&format!("Search engine: {}", engine.label())),
                );
                for tab in browser.tabs.borrow().iter() {
                    if tab.uri == HOME_URL {
                        tab.view.load_uri(HOME_URI);
                    }
                }
                browser.status.set_text(&format!(
                    "{} is now the search engine for address-bar and new-tab searches.",
                    engine.label()
                ));
            }
            close_menu.popdown();
        });
        content.append(&choice);
    }
    popover.set_child(Some(&content));
    popover.connect_closed(|popover| popover.unparent());
    popover.popup();
}

fn add_tab(browser: &Rc<Browser>, target: Option<&str>) {
    let id = browser.next_tab_id.get();
    browser.next_tab_id.set(id.wrapping_add(1).max(1));
    let view = WebView::builder()
        .web_context(&browser.context)
        .network_session(&browser.network_session)
        .build();
    view.set_hexpand(true);
    view.set_vexpand(true);
    let settings = WebViewExt::settings(&view).unwrap_or_default();
    settings.set_enable_javascript(browser.scripts_enabled.get());
    view.set_settings(&settings);

    let tab_container = GtkBox::new(Orientation::Horizontal, 0);
    tab_container.add_css_class("tab-shell");
    let title_label = Label::new(Some("New Tab"));
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_max_width_chars(24);
    title_label.set_xalign(0.0);
    title_label.set_halign(gtk::Align::Fill);
    title_label.set_hexpand(true);
    let tab_contents = GtkBox::new(Orientation::Horizontal, 9);
    tab_contents.add_css_class("tab-item");
    tab_contents.set_hexpand(true);
    let tab_logo = GtkBox::new(Orientation::Horizontal, 0);
    tab_logo.add_css_class("tab-logo-badge");
    tab_logo.set_size_request(22, 22);
    tab_logo.set_halign(gtk::Align::Start);
    tab_logo.set_valign(gtk::Align::Center);
    let linux_logo = gtk::Image::from_pixbuf(linux_pixbuf(15).as_ref());
    linux_logo.set_halign(gtk::Align::Center);
    linux_logo.set_valign(gtk::Align::Center);
    tab_logo.append(&linux_logo);
    tab_contents.append(&tab_logo);
    tab_contents.append(&title_label);
    let close = Button::with_label("×");
    close.add_css_class("tab-close");
    close.set_tooltip_text(Some("Close tab"));
    tab_container.append(&tab_contents);
    tab_container.append(&close);
    let page_num = browser.notebook.append_page(&view, Some(&tab_container));
    browser.notebook.set_tab_reorderable(&view, true);

    browser.tabs.borrow_mut().push(BrowserTab {
        id,
        view: view.clone(),
        tab_container: tab_container.clone(),
        tab_title: title_label.clone(),
        title: "New Tab".into(),
        uri: HOME_URL.into(),
    });
    browser.notebook.set_current_page(Some(page_num));
    activate_tab(browser, id);

    let weak = Rc::downgrade(browser);
    close.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade() {
            close_tab(&browser, id);
        }
    });

    let weak = Rc::downgrade(browser);
    view.connect_load_changed(move |web_view, event| {
        if let Some(browser) = weak.upgrade() {
            on_load_changed(&browser, id, web_view, event);
        }
    });
    let weak = Rc::downgrade(browser);
    view.connect_estimated_load_progress_notify(move |web_view| {
        if let Some(browser) = weak.upgrade()
            && browser.active_tab.get() == id
        {
            browser
                .progress
                .set_fraction(web_view.estimated_load_progress());
        }
    });
    let weak = Rc::downgrade(browser);
    view.connect_title_notify(move |web_view| {
        if let Some(browser) = weak.upgrade() {
            update_tab_title(&browser, id, web_view.title().as_deref());
        }
    });
    let weak = Rc::downgrade(browser);
    view.connect_uri_notify(move |web_view| {
        if let Some(browser) = weak.upgrade() {
            update_tab_uri(&browser, id, web_view.uri().as_deref());
        }
    });
    let weak = Rc::downgrade(browser);
    view.connect_load_failed(move |_, _, uri, error| {
        if let Some(browser) = weak.upgrade()
            && browser.active_tab.get() == id
        {
            set_button_icon(&browser.reload, "view-refresh-symbolic");
            browser.reload.set_tooltip_text(Some("Reload"));
            browser
                .status
                .set_text(&format!("Could not load {uri}: {error}"));
            browser.progress.set_visible(false);
        }
        false
    });
    let weak = Rc::downgrade(browser);
    view.connect_decide_policy(move |_, decision, decision_type| {
        if decision_type == PolicyDecisionType::NavigationAction
            && let Some(navigation) = decision.downcast_ref::<NavigationPolicyDecision>()
            && let Some(uri) = navigation
                .navigation_action()
                .and_then(|action| action.request())
                .and_then(|request| request.uri())
            && uri.starts_with("about:")
            && uri != "about:blank"
        {
            if let Some(browser) = weak.upgrade() {
                navigate_tab_id(&browser, id, &uri);
            }
            decision.ignore();
            return true;
        }
        false
    });

    if let Some(target) = target {
        navigate_tab_id(
            browser,
            id,
            &normalize_address(target, browser.search_engine.get()),
        );
    } else {
        navigate_tab_id(browser, id, HOME_URL);
    }
}

fn on_load_changed(browser: &Rc<Browser>, id: u64, view: &WebView, event: LoadEvent) {
    if browser.active_tab.get() != id {
        return;
    }
    match event {
        LoadEvent::Started => {
            set_button_icon(&browser.reload, "process-stop-symbolic");
            browser.reload.set_tooltip_text(Some("Stop loading"));
            browser.progress.set_fraction(0.0);
            browser.progress.set_visible(true);
            browser.status.set_text("Loading page…");
        }
        LoadEvent::Committed => {
            if let Some(uri) = view.uri() {
                browser.address.set_text(address_text(uri.as_str()));
            }
            browser.back.set_sensitive(view.can_go_back());
            browser.forward.set_sensitive(view.can_go_forward());
        }
        LoadEvent::Finished => {
            set_button_icon(&browser.reload, "view-refresh-symbolic");
            browser.reload.set_tooltip_text(Some("Reload"));
            browser.progress.set_visible(false);
            if let Some(uri) = view.uri() {
                browser.address.set_text(address_text(uri.as_str()));
            }
            browser.back.set_sensitive(view.can_go_back());
            browser.forward.set_sensitive(view.can_go_forward());
            browser.status.set_text("Page loaded · private session");
            refresh_bookmark_button(browser);
        }
        _ => {}
    }
}

fn update_tab_title(browser: &Rc<Browser>, id: u64, title: Option<&str>) {
    let Some(title) = title.filter(|title| !title.trim().is_empty()) else {
        return;
    };
    if let Some(tab) = browser
        .tabs
        .borrow_mut()
        .iter_mut()
        .find(|tab| tab.id == id)
    {
        tab.title = title.to_string();
        tab.tab_title.set_text(title);
        if browser.active_tab.get() == id {
            browser
                .window
                .set_title(Some(&format!("{title} — Linus Browser")));
        }
    }
}

fn update_tab_uri(browser: &Rc<Browser>, id: u64, uri: Option<&str>) {
    let Some(uri) = uri else { return };
    let uri = display_uri(uri);
    if let Some(tab) = browser
        .tabs
        .borrow_mut()
        .iter_mut()
        .find(|tab| tab.id == id)
    {
        tab.uri = uri.to_string();
        if browser.active_tab.get() == id {
            browser.address.set_text(address_text(uri));
        }
    }
}

fn activate_tab(browser: &Rc<Browser>, id: u64) {
    let tabs = browser.tabs.borrow();
    let Some(tab) = tabs.iter().find(|tab| tab.id == id) else {
        return;
    };
    browser.active_tab.set(id);
    if let Some(page_num) = browser.notebook.page_num(&tab.view) {
        browser.notebook.set_current_page(Some(page_num));
    }
    browser.address.set_text(address_text(&tab.uri));
    browser
        .window
        .set_title(Some(&format!("{} — Linus Browser", tab.title)));
    browser.back.set_sensitive(tab.view.can_go_back());
    browser.forward.set_sensitive(tab.view.can_go_forward());
    refresh_bookmark_button(browser);
    for tab in tabs.iter() {
        if tab.id == id {
            tab.tab_container.add_css_class("active");
        } else {
            tab.tab_container.remove_css_class("active");
        }
    }
}

fn close_tab(browser: &Rc<Browser>, id: u64) {
    let Some((index, page_num, was_active)) = browser
        .tabs
        .borrow()
        .iter()
        .enumerate()
        .find(|(_, tab)| tab.id == id)
        .and_then(|(index, tab)| {
            browser
                .notebook
                .page_num(&tab.view)
                .map(|page_num| (index, page_num, tab.id == browser.active_tab.get()))
        })
    else {
        return;
    };
    browser.notebook.remove_page(Some(page_num));
    browser.tabs.borrow_mut().remove(index);
    let tabs = browser.tabs.borrow();
    if tabs.is_empty() {
        drop(tabs);
        add_tab(browser, None);
    } else if was_active {
        let next = browser
            .notebook
            .nth_page(browser.notebook.current_page())
            .and_then(|page| page.downcast::<WebView>().ok())
            .and_then(|view| tabs.iter().find(|tab| tab.view == view).map(|tab| tab.id));
        drop(tabs);
        if let Some(next) = next {
            activate_tab(browser, next);
        }
    } else {
        drop(tabs);
        sync_navigation(browser);
    }
}

fn active_tab(browser: &Browser) -> Option<BrowserTabRef> {
    browser
        .tabs
        .borrow()
        .iter()
        .find(|tab| tab.id == browser.active_tab.get())
        .map(|tab| BrowserTabRef {
            view: tab.view.clone(),
            uri: tab.uri.clone(),
            title: tab.title.clone(),
        })
}

struct BrowserTabRef {
    view: WebView,
    uri: String,
    title: String,
}

fn navigate_active(browser: &Rc<Browser>, target: &str) {
    navigate_tab_id(browser, browser.active_tab.get(), target);
}

fn navigate_tab_id(browser: &Rc<Browser>, id: u64, target: &str) {
    let Some(view) = browser
        .tabs
        .borrow()
        .iter()
        .find(|tab| tab.id == id)
        .map(|tab| tab.view.clone())
    else {
        return;
    };
    if let Some(tab) = browser
        .tabs
        .borrow_mut()
        .iter_mut()
        .find(|tab| tab.id == id)
    {
        tab.uri = target.to_string();
        if target == HOME_URL {
            tab.title = "New Tab".into();
            tab.tab_title.set_text("New Tab");
        } else if target == PRIVACY_URL {
            tab.title = "Privacy".into();
            tab.tab_title.set_text("Privacy");
        }
    }
    if browser.active_tab.get() == id {
        browser.address.set_text(target);
    }
    match target {
        HOME_URL => view.load_uri(HOME_URI),
        PRIVACY_URL => view.load_uri(PRIVACY_URI),
        _ => view.load_uri(target),
    }
}

fn display_uri(uri: &str) -> &str {
    match uri {
        HOME_URI => HOME_URL,
        PRIVACY_URI => PRIVACY_URL,
        _ => uri,
    }
}

fn sync_navigation(browser: &Rc<Browser>) {
    if let Some(tab) = active_tab(browser) {
        browser.back.set_sensitive(tab.view.can_go_back());
        browser.forward.set_sensitive(tab.view.can_go_forward());
    }
}

fn toggle_bookmark(browser: &Rc<Browser>) {
    let Some(tab) = active_tab(browser) else {
        return;
    };
    if !tab.uri.starts_with("http://") && !tab.uri.starts_with("https://") {
        browser.status.set_text("Only web pages can be bookmarked.");
        return;
    }
    let mut bookmarks = browser.bookmarks.borrow_mut();
    if let Some(position) = bookmarks
        .iter()
        .position(|bookmark| bookmark.url == tab.uri)
    {
        bookmarks.remove(position);
        browser.status.set_text("Bookmark removed.");
    } else {
        bookmarks.push(Bookmark {
            title: tab.title,
            url: tab.uri,
        });
        browser.status.set_text("Bookmark saved.");
    }
    if let Err(error) = save_bookmarks(&browser.bookmarks_path, &bookmarks) {
        browser
            .status
            .set_text(&format!("Could not save bookmark: {error}"));
    }
    drop(bookmarks);
    refresh_bookmark_button(browser);
    rebuild_bookmark_bar(browser);
}

fn refresh_bookmark_button(browser: &Browser) {
    let Some(tab) = active_tab(browser) else {
        return;
    };
    let saved = browser
        .bookmarks
        .borrow()
        .iter()
        .any(|bookmark| bookmark.url == tab.uri);
    browser.bookmark.set_label(if saved { "★" } else { "☆" });
}

fn rebuild_bookmark_bar(browser: &Rc<Browser>) {
    let mut child = browser.bookmark_bar.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        browser.bookmark_bar.remove(&widget);
    }
    for (index, bookmark) in browser.bookmarks.borrow().iter().enumerate() {
        let button = Button::with_label(&bookmark.title);
        button.set_tooltip_text(Some(&bookmark.url));
        button.add_css_class("bookmark-item");
        let weak = Rc::downgrade(browser);
        button.connect_clicked(move |_| {
            if let Some(browser) = weak.upgrade()
                && let Some(bookmark) = browser.bookmarks.borrow().get(index)
            {
                let url = bookmark.url.clone();
                navigate_active(&browser, &url);
            }
        });
        browser.bookmark_bar.append(&button);
    }
}

fn show_browser_menu(browser: &Rc<Browser>, relative_to: &Button) {
    let popover = gtk::Popover::new();
    popover.set_parent(relative_to);
    popover.add_css_class("popover");
    let content = GtkBox::new(Orientation::Vertical, 6);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);
    let appearance_title = Label::new(Some("Appearance"));
    appearance_title.set_xalign(0.0);
    appearance_title.add_css_class("menu-heading");
    content.append(&appearance_title);
    for mode in [
        AppearanceMode::System,
        AppearanceMode::Dark,
        AppearanceMode::Light,
    ] {
        let label = if browser.appearance.get() == mode {
            format!("✓  {}", mode.label())
        } else {
            format!("     {}", mode.label())
        };
        let choice = Button::with_label(&label);
        choice.set_halign(gtk::Align::Fill);
        choice.add_css_class("engine-choice");
        let weak = Rc::downgrade(browser);
        let close_menu = popover.clone();
        choice.connect_clicked(move |_| {
            if let Some(browser) = weak.upgrade() {
                set_appearance(&browser, mode);
            }
            close_menu.popdown();
        });
        content.append(&choice);
    }
    let wallpaper_title = Label::new(Some("New-tab background"));
    wallpaper_title.set_xalign(0.0);
    wallpaper_title.add_css_class("menu-heading");
    content.append(&wallpaper_title);
    let choose_wallpaper = Button::with_label("Choose image…");
    choose_wallpaper.set_halign(gtk::Align::Fill);
    choose_wallpaper.add_css_class("engine-choice");
    let weak = Rc::downgrade(browser);
    let close_menu = popover.clone();
    choose_wallpaper.connect_clicked(move |_| {
        close_menu.popdown();
        if let Some(browser) = weak.upgrade() {
            show_wallpaper_chooser(&browser);
        }
    });
    content.append(&choose_wallpaper);
    let reset_wallpaper = Button::with_label("Use default background");
    reset_wallpaper.set_halign(gtk::Align::Fill);
    reset_wallpaper.add_css_class("engine-choice");
    reset_wallpaper.set_sensitive(browser.new_tab_wallpaper.borrow().path.is_some());
    let weak = Rc::downgrade(browser);
    let close_menu = popover.clone();
    reset_wallpaper.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade()
            && let Err(error) = reset_new_tab_wallpaper(&browser)
        {
            browser
                .status
                .set_text(&format!("Could not restore default background: {error}"));
        }
        close_menu.popdown();
    });
    content.append(&reset_wallpaper);
    let bookmarks = Button::with_label("Show bookmarks");
    let show = browser.bookmark_revealer.reveals_child();
    bookmarks.set_label(if show {
        "Hide bookmarks"
    } else {
        "Show bookmarks"
    });
    let weak = Rc::downgrade(browser);
    let popover_for_bookmarks = popover.clone();
    bookmarks.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade() {
            let reveal = !browser.bookmark_revealer.reveals_child();
            browser.bookmark_revealer.set_reveal_child(reveal);
        }
        popover_for_bookmarks.popdown();
    });
    let scripts = ToggleButton::with_label("Allow JavaScript");
    scripts.set_active(browser.scripts_enabled.get());
    let weak = Rc::downgrade(browser);
    let popover_for_scripts = popover.clone();
    scripts.connect_toggled(move |button| {
        if let Some(browser) = weak.upgrade() {
            let enabled = button.is_active();
            browser.scripts_enabled.set(enabled);
            for tab in browser.tabs.borrow().iter() {
                if let Some(settings) = WebViewExt::settings(&tab.view) {
                    settings.set_enable_javascript(enabled);
                }
            }
            browser.status.set_text(if enabled {
                "JavaScript enabled. Reload pages to apply."
            } else {
                "JavaScript disabled. Reload pages to apply."
            });
        }
        popover_for_scripts.popdown();
    });
    let clear_data = Button::with_label("Clear cookies");
    let weak = Rc::downgrade(browser);
    let popover_for_clear = popover.clone();
    clear_data.connect_clicked(move |_| {
        if let Some(browser) = weak.upgrade() {
            if let Some(manager) = browser.network_session.website_data_manager() {
                clear_session_cookies(&browser.status, &manager);
            } else {
                browser.status.set_text("Could not access session cookies.");
            }
        }
        popover_for_clear.popdown();
    });
    content.append(&bookmarks);
    content.append(&scripts);
    content.append(&clear_data);
    popover.set_child(Some(&content));
    popover.connect_closed(|popover| popover.unparent());
    popover.popup();
}

fn show_wallpaper_chooser(browser: &Rc<Browser>) {
    let chooser = gtk::FileDialog::new();
    chooser.set_title("Choose new-tab background");
    let filter = FileFilter::new();
    filter.set_name(Some("Images (PNG, JPEG, WebP)"));
    filter.add_mime_type("image/png");
    filter.add_mime_type("image/jpeg");
    filter.add_mime_type("image/webp");
    let filters = gio::ListStore::new::<FileFilter>();
    filters.append(&filter);
    chooser.set_filters(Some(&filters));

    let weak = Rc::downgrade(browser);
    chooser.open(
        Some(&browser.window),
        None::<&gio::Cancellable>,
        move |result| {
            let Some(browser) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(file) => {
                    let Some(path) = file.path() else {
                        browser
                            .status
                            .set_text("Could not access the selected background image.");
                        return;
                    };
                    if let Err(error) = set_new_tab_wallpaper(&browser, &path) {
                        browser
                            .status
                            .set_text(&format!("Could not set new-tab background: {error}"));
                    }
                }
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => browser
                    .status
                    .set_text(&format!("Could not open background image: {error}")),
            }
        },
    );
}

fn set_shields(browser: &Rc<Browser>, enabled: bool) {
    let policy = if enabled {
        CookieAcceptPolicy::NoThirdParty
    } else {
        CookieAcceptPolicy::Always
    };
    if let Some(manager) = browser.network_session.cookie_manager() {
        manager.set_accept_policy(policy);
    }
    browser.shield.set_tooltip_text(Some(if enabled {
        "Privacy shield: block third-party cookies"
    } else {
        "Privacy shield off: allow cookies for this session"
    }));
    browser.status.set_text(if enabled {
        "Shields on · third-party cookies blocked · data stays in memory"
    } else {
        "Shields off · cookies allowed for this session only"
    });
}

fn clear_session_cookies(status: &Label, manager: &WebsiteDataManager) {
    let weak_status: glib::SendWeakRef<Label> = status.downgrade().into();
    status.set_text("Clearing session cookies…");
    manager.clear(
        WebsiteDataTypes::COOKIES,
        glib::TimeSpan::from_seconds(0),
        None::<&gio::Cancellable>,
        move |result| {
            glib::MainContext::default().invoke(move || {
                if let Some(status) = weak_status.upgrade() {
                    status.set_text(match result {
                        Ok(()) => "Session cookies cleared.",
                        Err(error) => {
                            eprintln!("Could not clear session cookies: {error}");
                            "Could not clear session cookies; see the application log."
                        }
                    });
                }
            });
        },
    );
}

fn connect_downloads(browser: &Rc<Browser>) {
    let weak: Weak<Browser> = Rc::downgrade(browser);
    browser
        .network_session
        .connect_download_started(move |_, download| {
            let Some(browser) = weak.upgrade() else {
                download.cancel();
                return;
            };
            let suggested = download
                .response()
                .and_then(|response| response.suggested_filename())
                .map(|name| name.to_string())
                .or_else(|| {
                    download
                        .request()
                        .and_then(|request| request.uri())
                        .and_then(|uri| {
                            Path::new(uri.as_str())
                                .file_name()
                                .map(|name| name.to_string_lossy().to_string())
                        })
                })
                .unwrap_or_else(|| "download".into());
            let path = match unique_download_path(&suggested) {
                Ok(path) => path,
                Err(error) => {
                    browser
                        .status
                        .set_text(&format!("Download failed: {error}"));
                    download.cancel();
                    return;
                }
            };
            let destination = gio::File::for_path(&path).uri();
            download.set_destination(destination.as_str());
            browser
                .status
                .set_text(&format!("Downloading {suggested}…"));
            let weak = Rc::downgrade(&browser);
            download.connect_finished(move |_| {
                if let Some(browser) = weak.upgrade() {
                    browser
                        .status
                        .set_text(&format!("Downloaded to {}", path.display()));
                }
            });
            let weak = Rc::downgrade(&browser);
            download.connect_failed(move |_, error| {
                if let Some(browser) = weak.upgrade() {
                    browser
                        .status
                        .set_text(&format!("Download failed: {error}"));
                }
            });
        });
}

fn normalize_address(input: &str, engine: SearchEngine) -> String {
    let input = input.trim();
    if input.is_empty() {
        return HOME_URL.into();
    }
    if input == HOME_URL || input == PRIVACY_URL {
        return input.into();
    }
    if input.starts_with("https://") || input.starts_with("http://") {
        return input.into();
    }
    if input.contains("://") {
        return engine.search_url(input);
    }
    let has_search_chars = input.chars().any(char::is_whitespace);
    let is_local_host = input == "localhost"
        || input.starts_with("localhost:")
        || input == "127.0.0.1"
        || input.starts_with("127.0.0.1:")
        || input == "[::1]"
        || input.starts_with("[::1]:");
    let looks_like_host = input.contains('.') || is_local_host;
    if has_search_chars || !looks_like_host {
        engine.search_url(input)
    } else if is_local_host {
        format!("http://{input}")
    } else {
        format!("https://{input}")
    }
}

fn percent_encode(input: &str) -> String {
    let mut output = String::new();
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn bookmarks_path() -> PathBuf {
    data_directory()
        .join("linus-browser")
        .join("bookmarks.json")
}

fn data_directory() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/share")
        })
}

fn default_new_tab_wallpaper() -> NewTabWallpaper {
    NewTabWallpaper {
        path: None,
        data_url: format!(
            "data:image/png;base64,{}",
            base64_encode(include_bytes!("assets/brave-red-wallpaper.png"))
        ),
    }
}

fn load_new_tab_wallpaper(path: Option<&Path>) -> Result<NewTabWallpaper, String> {
    let Some(path) = path else {
        return Ok(default_new_tab_wallpaper());
    };
    let (_, mime_type) = wallpaper_image_type(path)?;
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(NewTabWallpaper {
        path: Some(path.to_path_buf()),
        data_url: format!("data:{mime_type};base64,{}", base64_encode(&bytes)),
    })
}

fn wallpaper_image_type(path: &Path) -> Result<(&'static str, &'static str), String> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok(("png", "image/png")),
        Some("jpg" | "jpeg") => Ok(("jpg", "image/jpeg")),
        Some("webp") => Ok(("webp", "image/webp")),
        _ => Err("choose an image in PNG, JPEG, or WebP format".into()),
    }
}

fn persist_new_tab_wallpaper(source: &Path) -> Result<(PathBuf, String), String> {
    let (extension, mime_type) = wallpaper_image_type(source)?;
    let bytes = fs::read(source).map_err(|error| format!("{}: {error}", source.display()))?;
    let data_url = format!("data:{mime_type};base64,{}", base64_encode(&bytes));
    let directory = data_directory().join("linus-browser");
    fs::create_dir_all(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let (path, temporary) = loop {
        let id = NEXT_WALLPAPER_ID.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(
            "new-tab-background-{}-{id}.{extension}",
            std::process::id()
        ));
        let temporary = directory.join(format!(
            "new-tab-background-{}-{id}.{extension}.tmp",
            std::process::id()
        ));
        if !path.exists() && !temporary.exists() {
            break (path, temporary);
        }
    };
    if let Err(error) = fs::write(&temporary, &bytes) {
        let cleanup_error = fs::remove_file(&temporary).err();
        return Err(match cleanup_error {
            Some(cleanup_error) => format!(
                "{}: {error}; could not remove temporary file {}: {cleanup_error}",
                temporary.display(),
                temporary.display()
            ),
            None => format!("{}: {error}", temporary.display()),
        });
    }
    if let Err(error) = fs::rename(&temporary, &path) {
        let cleanup_error = fs::remove_file(&temporary).err();
        return Err(match cleanup_error {
            Some(cleanup_error) => format!(
                "could not save background to {}: {error}; could not remove temporary file {}: {cleanup_error}",
                path.display(),
                temporary.display()
            ),
            None => format!("could not save background to {}: {error}", path.display()),
        });
    }
    Ok((path, data_url))
}

fn preferences_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        })
        .join("linus-browser")
        .join("preferences.json")
}

fn load_preferences(path: &Path) -> Result<BrowserPreferences, String> {
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map_err(|error| format!("{}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(BrowserPreferences::default())
        }
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn save_preferences(path: &Path, preferences: &BrowserPreferences) -> Result<(), String> {
    let parent = path.parent().ok_or("preferences path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(preferences).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn load_bookmarks(path: &Path) -> Result<Vec<Bookmark>, String> {
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map_err(|error| format!("{}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn save_bookmarks(path: &Path, bookmarks: &[Bookmark]) -> Result<(), String> {
    let parent = path.parent().ok_or("bookmark path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(bookmarks).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn unique_download_path(suggested: &str) -> Result<PathBuf, String> {
    let downloads = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Downloads");
    fs::create_dir_all(&downloads).map_err(|error| {
        format!(
            "could not create download directory {}: {error}",
            downloads.display()
        )
    })?;
    let safe_name: String = Path::new(suggested)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(120)
        .collect();
    let name = if safe_name.is_empty() || safe_name == "." || safe_name == ".." {
        "download".to_string()
    } else {
        safe_name
    };
    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or("download");
    let extension = path.extension().and_then(|part| part.to_str());
    for index in 0..10_000 {
        let candidate = if index == 0 {
            downloads.join(&name)
        } else if let Some(extension) = extension {
            downloads.join(format!("{stem} ({index}).{extension}"))
        } else {
            downloads.join(format!("{stem} ({index})"))
        };
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Ok(downloads.join(format!("{name}-{}", std::process::id())))
}

fn icon_button(icon: &str, tooltip: &str) -> Button {
    let button = if icon.ends_with("-symbolic") {
        let button = Button::new();
        set_button_icon(&button, icon);
        button
    } else {
        Button::with_label(icon)
    };
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("nav-button");
    button
}

fn set_button_icon(button: &Button, icon: &str) {
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(18);
    button.set_child(Some(&image));
}

fn home_html(engine: SearchEngine, dark: bool, wallpaper_data: &str) -> String {
    let logo = format!(
        "data:image/png;base64,{}",
        base64_encode(include_bytes!("assets/tux.png"))
    );
    HOME_HTML
        .replace("{{TUX_DATA}}", &logo)
        .replace("{{WALLPAPER_DATA}}", wallpaper_data)
        .replace("{{SEARCH_PREFIX}}", engine.search_prefix())
        .replace("{{THEME_CLASS}}", if dark { "dark" } else { "light" })
}

fn privacy_html(dark: bool) -> String {
    PRIVACY_HTML.replace("{{THEME_CLASS}}", if dark { "dark" } else { "light" })
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        encoded.push(TABLE[(a >> 2) as usize] as char);
        encoded.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(c & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

fn linux_pixbuf(size: i32) -> Option<gtk::gdk_pixbuf::Pixbuf> {
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    if let Err(error) = loader.write(include_bytes!("assets/linux-tux.png")) {
        eprintln!("Could not load Linux tab logo: {error}");
        return None;
    }
    if let Err(error) = loader.close() {
        eprintln!("Could not finish loading Linux tab logo: {error}");
        return None;
    }
    let pixbuf = loader.pixbuf()?;
    pixbuf.scale_simple(size, size, gtk::gdk_pixbuf::InterpType::Bilinear)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_input_distinguishes_searches_from_sites() {
        assert_eq!(
            normalize_address("rust browser", SearchEngine::default()),
            "https://duckduckgo.com/?q=rust%20browser"
        );
        assert_eq!(
            normalize_address("example.org", SearchEngine::default()),
            "https://example.org"
        );
        assert_eq!(
            normalize_address("https://example.org", SearchEngine::default()),
            "https://example.org"
        );
        assert_eq!(
            normalize_address("about:home", SearchEngine::default()),
            HOME_URL
        );
        assert_eq!(
            normalize_address("127.0.0.1", SearchEngine::default()),
            "http://127.0.0.1"
        );
        assert_eq!(
            normalize_address("localhost:8080", SearchEngine::default()),
            "http://localhost:8080"
        );
    }

    #[test]
    fn search_engines_build_encoded_search_urls() {
        assert_eq!(
            SearchEngine::DuckDuckGo.search_url("linus browser"),
            "https://duckduckgo.com/?q=linus%20browser"
        );
        assert_eq!(
            normalize_address("rust browser", SearchEngine::Google),
            "https://www.google.com/search?q=rust%20browser"
        );
        assert_eq!(
            normalize_address("rust browser", SearchEngine::Wikipedia),
            "https://en.wikipedia.org/w/index.php?search=rust%20browser"
        );
    }

    #[test]
    fn new_tab_page_uses_the_selected_search_engine() {
        let wallpaper = default_new_tab_wallpaper().data_url;
        let duckduckgo = home_html(SearchEngine::DuckDuckGo, true, &wallpaper);
        assert!(duckduckgo.contains("https://duckduckgo.com/?q="));
        assert!(duckduckgo.contains("data:image/png;base64,"));
        assert!(!duckduckgo.contains("{{WALLPAPER_DATA}}"));
        assert!(duckduckgo.contains("<body class=\"dark\">"));
        assert!(!duckduckgo.contains("search.brave.com"));
        assert!(!duckduckgo.contains("private by design"));
        assert!(!duckduckgo.contains("class=\"credit\""));

        let google = home_html(SearchEngine::Google, false, &wallpaper);
        assert!(google.contains("https://www.google.com/search?q="));
        assert!(google.contains("<body class=\"light\">"));
        let wikipedia = home_html(SearchEngine::Wikipedia, true, &wallpaper);
        assert!(wikipedia.contains("https://en.wikipedia.org/w/index.php?search="));
    }

    #[test]
    fn appearance_preference_round_trips() {
        assert_eq!(
            BrowserPreferences::default().appearance,
            AppearanceMode::Dark
        );
        assert!(BrowserPreferences::default().wallpaper.is_none());
        for appearance in [
            AppearanceMode::System,
            AppearanceMode::Dark,
            AppearanceMode::Light,
        ] {
            let preferences = BrowserPreferences {
                appearance,
                wallpaper: Some(PathBuf::from("/tmp/new-tab.png")),
            };
            let json = serde_json::to_vec(&preferences).unwrap();
            let loaded: BrowserPreferences = serde_json::from_slice(&json).unwrap();
            assert_eq!(loaded.appearance, appearance);
            assert_eq!(loaded.wallpaper, preferences.wallpaper);
        }
    }

    #[test]
    fn old_preferences_without_wallpaper_still_load() {
        let loaded: BrowserPreferences = serde_json::from_str(r#"{"appearance":"Dark"}"#).unwrap();
        assert_eq!(loaded.appearance, AppearanceMode::Dark);
        assert!(loaded.wallpaper.is_none());
    }

    #[test]
    fn bookmark_storage_round_trips() {
        let bookmark = Bookmark {
            title: "Example".into(),
            url: "https://example.org".into(),
        };
        let json = serde_json::to_vec(&vec![bookmark.clone()]).unwrap();
        let loaded: Vec<Bookmark> = serde_json::from_slice(&json).unwrap();
        assert_eq!(loaded, vec![bookmark]);
    }

    #[test]
    fn webkit_loads_a_real_http_page() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        if gtk::init().is_err() {
            return;
        }

        let (_, dark_css_error) = install_css(AppearanceMode::Dark);
        assert!(
            dark_css_error.is_none(),
            "Dark GTK CSS failed: {dark_css_error:?}"
        );
        let (_, light_css_error) = install_css(AppearanceMode::Light);
        assert!(
            light_css_error.is_none(),
            "Light GTK CSS failed: {light_css_error:?}"
        );

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let body = "<!doctype html><title>WebKit verified</title><h1>Internet page</h1>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body,
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let context = WebContext::new();
        let network_session = NetworkSession::new_ephemeral();
        register_internal_scheme(
            &context,
            Rc::new(Cell::new(SearchEngine::default())),
            Rc::new(Cell::new(AppearanceMode::Dark)),
            Rc::new(RefCell::new(default_new_tab_wallpaper())),
        );
        let view = WebView::builder()
            .web_context(&context)
            .network_session(&network_session)
            .build();
        let window = gtk::Window::new();
        window.set_child(Some(&view));
        let wait_for_load = |view: &WebView, window: &gtk::Window, uri: &str| {
            let main_loop = glib::MainLoop::new(None, false);
            let finished = Rc::new(Cell::new(false));
            let completed = main_loop.clone();
            let finished_on_load = finished.clone();
            view.connect_load_changed(move |web_view, event| {
                if event == LoadEvent::Finished {
                    finished_on_load.set(true);
                    if web_view.title().is_some() {
                        completed.quit();
                    }
                }
            });
            let completed = main_loop.clone();
            view.connect_title_notify(move |web_view| {
                if finished.get() && web_view.title().is_some() {
                    completed.quit();
                }
            });
            let timeout = main_loop.clone();
            let timeout_id = glib::timeout_add_seconds_local(12, move || {
                timeout.quit();
                glib::ControlFlow::Break
            });
            view.load_uri(uri);
            window.present();
            main_loop.run();
            timeout_id.remove();
        };
        wait_for_load(&view, &window, HOME_URI);
        assert_eq!(view.title().as_deref(), Some("New Tab"));
        window.close();

        let network_view = WebView::builder()
            .web_context(&context)
            .network_session(&network_session)
            .build();
        let network_window = gtk::Window::new();
        network_window.set_child(Some(&network_view));
        wait_for_load(
            &network_view,
            &network_window,
            &format!("http://{address}/"),
        );
        server.join().unwrap();
        network_window.close();

        assert_eq!(network_view.title().as_deref(), Some("WebKit verified"));
    }
}
