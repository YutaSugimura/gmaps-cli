// Fetch the current location via CoreLocation.
//
// NOTE: macOS TCC (location permission) is managed **per .app bundle**.
// A bare CLI binary will never see the authorization dialog, leaving the
// status as notDetermined until it times out. Wrapping the binary into an
// .app via cargo-bundle (or similar) is required for this to work.

use super::LatLng;
use anyhow::{Result, anyhow, bail};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};
use objc2_core_location::{
    CLAuthorizationStatus, CLLocation, CLLocationManager, CLLocationManagerDelegate,
};
use objc2_foundation::{NSArray, NSDate, NSError, NSObject, NSObjectProtocol, NSRunLoop};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TIMEOUT_SECS: u64 = 15;

#[derive(Debug)]
enum GpsState {
    Pending,
    Success(LatLng),
    Failure(String),
}

struct DelegateIvars {
    state: Mutex<GpsState>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MapCliLocationDelegate"]
    #[ivars = DelegateIvars]
    struct LocationDelegate;

    unsafe impl NSObjectProtocol for LocationDelegate {}

    unsafe impl CLLocationManagerDelegate for LocationDelegate {
        #[unsafe(method(locationManager:didUpdateLocations:))]
        fn did_update_locations(
            &self,
            _manager: &CLLocationManager,
            locations: &NSArray<CLLocation>,
        ) {
            let count = locations.count();
            if count == 0 {
                return;
            }
            let loc = locations.objectAtIndex(count - 1);
            // Skip readings before the accuracy has settled.
            let accuracy = unsafe { loc.horizontalAccuracy() };
            if !(0.0..=1000.0).contains(&accuracy) {
                return;
            }
            let coord = unsafe { loc.coordinate() };
            let mut s = self.ivars().state.lock().unwrap();
            if matches!(*s, GpsState::Pending) {
                *s = GpsState::Success(LatLng {
                    lat: coord.latitude,
                    lng: coord.longitude,
                });
            }
        }

        #[unsafe(method(locationManager:didFailWithError:))]
        fn did_fail_with_error(&self, _manager: &CLLocationManager, error: &NSError) {
            let code = error.code();
            // kCLErrorLocationUnknown (1) is a transient error; keep waiting.
            if code == 1 {
                return;
            }
            let desc = error.localizedDescription();
            let mut s = self.ivars().state.lock().unwrap();
            if matches!(*s, GpsState::Pending) {
                *s = GpsState::Failure(format!("CoreLocation error (code {code}): {desc}"));
            }
        }

        #[unsafe(method(locationManagerDidChangeAuthorization:))]
        fn did_change_authorization(&self, manager: &CLLocationManager) {
            let status = unsafe { manager.authorizationStatus() };
            match status {
                CLAuthorizationStatus::Restricted | CLAuthorizationStatus::Denied => {
                    let mut s = self.ivars().state.lock().unwrap();
                    if matches!(*s, GpsState::Pending) {
                        *s = GpsState::Failure(
                            "Location permission denied. Allow gmaps under System Settings > Privacy & Security > Location Services.".into()
                        );
                    }
                }
                CLAuthorizationStatus::AuthorizedAlways
                | CLAuthorizationStatus::AuthorizedWhenInUse => {
                    unsafe { manager.startUpdatingLocation() };
                }
                _ => {} // notDetermined: keep waiting.
            }
        }
    }
);

impl LocationDelegate {
    fn new() -> Retained<Self> {
        let alloc = LocationDelegate::alloc().set_ivars(DelegateIvars {
            state: Mutex::new(GpsState::Pending),
        });
        unsafe { msg_send![super(alloc), init] }
    }
}

pub fn run() -> Result<LatLng> {
    let delegate = LocationDelegate::new();
    let delegate_proto: Retained<ProtocolObject<dyn CLLocationManagerDelegate>> =
        ProtocolObject::from_retained(delegate.clone());

    let manager: Retained<CLLocationManager> = unsafe { CLLocationManager::new() };
    unsafe {
        manager.setDelegate(Some(&delegate_proto));
        manager.requestWhenInUseAuthorization();
        manager.startUpdatingLocation();
    }

    // Spin the main run loop in small slices until the deadline.
    let run_loop = NSRunLoop::currentRunLoop();
    let deadline = Instant::now() + Duration::from_secs(TIMEOUT_SECS);
    loop {
        let until = NSDate::dateWithTimeIntervalSinceNow(0.1);
        run_loop.runUntilDate(&until);

        let s = delegate.ivars().state.lock().unwrap();
        match &*s {
            GpsState::Success(ll) => {
                let result = *ll;
                drop(s);
                unsafe { manager.stopUpdatingLocation() };
                return Ok(result);
            }
            GpsState::Failure(msg) => {
                let err = msg.clone();
                drop(s);
                unsafe { manager.stopUpdatingLocation() };
                bail!(err);
            }
            GpsState::Pending => {}
        }

        if Instant::now() >= deadline {
            unsafe { manager.stopUpdatingLocation() };
            return Err(anyhow!(
                "GPS lookup timed out.\n  → If no authorization dialog appeared, the binary may not be inside an .app bundle.\n  → Run 'cargo bundle --release' to wrap it as a .app and try again."
            ));
        }
    }
}
