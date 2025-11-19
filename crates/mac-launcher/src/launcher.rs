use std::sync::LazyLock;

use shared::appdata;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use objc2_foundation::{NSArray, NSNotification, NSString, NSURL};

use shared::log;

static UDSCLIENT: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    let appdata = appdata::AppData::load();
    appdata
        .launcher_path
        .unwrap_or_else(|| {
            // Should be on same dir as mac-launcer, if not in appdata
            let exe_path = std::env::current_exe().expect("Failed to get current exe path");
            exe_path
                .parent()
                .expect("Failed to get parent directory of exe")
                .join("launcher")
                .as_os_str()
                .to_string_lossy()
                .into_owned()
        })
        .into()
});

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notif: &NSNotification) {
            log::debug!("Launcher path is: {}", UDSCLIENT.as_path().display());
        }

        // Processs udssv2:// URLs
        #[unsafe(method(application:openURLs:))]
        fn application_open_urls(&self, _app: &NSApplication, urls: &NSArray<NSURL>) {
            for url in urls {
                let url = url
                    .absoluteString()
                    .unwrap_or_else(|| NSString::from_str(""))
                    .to_string();
                if !url.starts_with("udssv2://") {
                    log::error!("Invalid URL scheme: {}", url);
                    continue;
                }
                // Launch the UDS Launcher with the URL as argument, only first URL, discard the rest
                let launcher_path = UDSCLIENT.as_path();
                log::info!(
                    "Launching UDS Launcher at {} with URL {}",
                    launcher_path.display(),
                    url
                );
                match std::process::Command::new(launcher_path).arg(url).spawn() {
                    Ok(_child) => {
                        log::info!("UDS Launcher launched successfully");
                    }
                    Err(e) => {
                        log::error!("Failed to launch UDS Launcher: {}", e);
                    }
                }
                break; // Only process the first URL    
            }
        }
    }
);

fn new_delegate(mtm: MainThreadMarker) -> Retained<AppDelegate> {
    let alloc = mtm.alloc::<AppDelegate>();
    unsafe { msg_send![alloc, init] }
}

pub fn launch() {
    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);

    let delegate = Box::leak(Box::new(new_delegate(mtm)));
    let proto: &ProtocolObject<dyn NSApplicationDelegate> = ProtocolObject::from_ref(&**delegate);
    app.setDelegate(Some(proto));

    // Keep app running in background without dock icon
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    app.run();
}
