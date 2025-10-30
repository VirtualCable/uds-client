use std::cell::OnceCell;
use std::sync::LazyLock;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use objc2_foundation::{NSArray, NSNotification, NSString, NSURL};

static UDSCLIENT: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    std::env::var("UDSCLIENT_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/usr/local/bin/udsclient"))
});

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notif: &NSNotification) {
            // log_message("Application did finish launching");
        }

        // Para esquemas de URL (uds2://…)
        #[unsafe(method(application:openURLs:))]
        fn application_open_urls(&self, _app: &NSApplication, urls: &NSArray<NSURL>) {
            for url in urls {
                let _s = url
                    .absoluteString()
                    .unwrap_or_else(|| NSString::from_str(""))
                    .to_string();
                //log_message(&s);
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

    // Mantener la app viva aunque no tenga ventanas
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    app.run();
}
