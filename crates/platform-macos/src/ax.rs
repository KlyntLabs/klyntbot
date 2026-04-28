//! Safe RAII wrappers for macOS Accessibility API types.
//!
//! `AXUIElement` and `AXValue` implement [`core_foundation::base::TCFType`],
//! giving them automatic memory management (`Drop`), `Clone`, `PartialEq`, and
//! seamless interoperability with the rest of the CoreFoundation ecosystem.
//!
//! Before this module, every AX call site did manual `CFRelease` bookkeeping
//! with raw `*mut c_void` pointers. That made leaks inevitable on refactors,
//! early returns, or `?` propagation. The wrappers make leaks impossible at
//! compile time.

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use core_foundation::{declare_TCFType, impl_TCFType};
use core_graphics::geometry::{CGPoint, CGSize};
use platform_capture::CaptureError;
use std::ffi::c_void;

pub type AXUIElementRef = *mut c_void;
pub type AXValueRef = *mut c_void;

declare_TCFType!(AXUIElement, AXUIElementRef);
impl_TCFType!(AXUIElement, AXUIElementRef, AXUIElementGetTypeID);

declare_TCFType!(AXValue, AXValueRef);
impl_TCFType!(AXValue, AXValueRef, AXValueGetTypeID);

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFTypeRef,
        value: CFTypeRef,
    ) -> i32;
    fn AXUIElementGetTypeID() -> usize;
    fn AXValueCreate(value_type: u32, value: *const c_void) -> AXValueRef;
    fn AXValueGetValue(value: AXValueRef, the_type: u32, value_ptr: *mut c_void) -> i32;
    fn AXValueGetTypeID() -> usize;
}

const K_AX_VALUE_CG_POINT_TYPE: u32 = 1;
const K_AX_VALUE_CG_SIZE_TYPE: u32 = 2;
const AX_ERROR_SUCCESS: i32 = 0;

// ---------------------------------------------------------------------------
// AXUIElement helpers
// ---------------------------------------------------------------------------

impl AXUIElement {
    /// Create an `AXUIElement` for the application with the given PID.
    pub fn create_application(pid: i32) -> Option<Self> {
        let ptr = unsafe { AXUIElementCreateApplication(pid) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { AXUIElement::wrap_under_create_rule(ptr) })
        }
    }

    /// Copy an attribute value. The concrete CF type depends on the attribute.
    ///
    /// Use [`CFType::downcast_into`] to convert to `AXUIElement`, `CFString`,
    /// `CFArray`, `AXValue`, etc.
    pub fn copy_attribute(&self, name: &str) -> Result<CFType, CaptureError> {
        let key = CFString::new(name);
        let mut out: CFTypeRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyAttributeValue(
                self.as_concrete_TypeRef(),
                key.as_CFTypeRef(),
                &mut out,
            )
        };
        if err != AX_ERROR_SUCCESS || out.is_null() {
            return Err(CaptureError::AxTreeUnavailable(format!(
                "AXUIElementCopyAttributeValue({name}) failed: {err}"
            )));
        }
        Ok(unsafe { CFType::wrap_under_create_rule(out) })
    }

    /// Set an attribute to a CoreFoundation value.
    pub fn set_attribute(&self, name: &str, value: CFTypeRef) -> Result<(), CaptureError> {
        let key = CFString::new(name);
        let err = unsafe {
            AXUIElementSetAttributeValue(self.as_concrete_TypeRef(), key.as_CFTypeRef(), value)
        };
        if err != AX_ERROR_SUCCESS {
            return Err(CaptureError::AxTreeUnavailable(format!(
                "AXUIElementSetAttributeValue({name}) failed: {err}"
            )));
        }
        Ok(())
    }

    /// Convenience: copy a string attribute (e.g. `AXTitle`, `AXRole`).
    pub fn copy_string_attribute(&self, name: &str) -> Result<String, CaptureError> {
        let value = self.copy_attribute(name)?;
        let cfstr = value.downcast_into::<CFString>().ok_or_else(|| {
            CaptureError::AxTreeUnavailable(format!("{name} was not a CFString"))
        })?;
        Ok(cfstr.to_string())
    }

    /// Convenience: get `AXChildren` as a `Vec` of owned `AXUIElement`s.
    ///
    /// Each child is automatically released when the `AXUIElement` is dropped.
    pub fn children(&self) -> Result<Vec<AXUIElement>, CaptureError> {
        let value = self.copy_attribute("AXChildren")?;
        let array: CFArray<CFTypeRef> = value.downcast_into().ok_or_else(|| {
            CaptureError::AxTreeUnavailable("AXChildren was not a CFArray".into())
        })?;

        let mut out = Vec::with_capacity(array.len() as usize);
        for i in 0..array.len() {
            if let Some(item) = array.get(i) {
                // `item` is `&CFTypeRef`. `wrap_under_get_rule` retains it.
                let elem = unsafe { AXUIElement::wrap_under_get_rule(*item as AXUIElementRef) };
                out.push(elem);
            }
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// AXValue helpers
// ---------------------------------------------------------------------------

impl AXValue {
    /// Create an `AXValue` from a `CGPoint`.
    pub fn from_point(point: CGPoint) -> Option<Self> {
        let ptr = unsafe {
            AXValueCreate(K_AX_VALUE_CG_POINT_TYPE, &point as *const _ as *const c_void)
        };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { AXValue::wrap_under_create_rule(ptr) })
        }
    }

    /// Create an `AXValue` from a `CGSize`.
    pub fn from_size(size: CGSize) -> Option<Self> {
        let ptr = unsafe {
            AXValueCreate(K_AX_VALUE_CG_SIZE_TYPE, &size as *const _ as *const c_void)
        };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { AXValue::wrap_under_create_rule(ptr) })
        }
    }

    /// Read the wrapped value as a `CGPoint`.
    pub fn to_point(&self) -> Option<CGPoint> {
        let mut point = CGPoint::new(0.0, 0.0);
        let ok = unsafe {
            AXValueGetValue(
                self.as_concrete_TypeRef(),
                K_AX_VALUE_CG_POINT_TYPE,
                &mut point as *mut _ as *mut c_void,
            )
        };
        if ok != 0 {
            Some(point)
        } else {
            None
        }
    }

    /// Read the wrapped value as a `CGSize`.
    pub fn to_size(&self) -> Option<CGSize> {
        let mut size = CGSize::new(0.0, 0.0);
        let ok = unsafe {
            AXValueGetValue(
                self.as_concrete_TypeRef(),
                K_AX_VALUE_CG_SIZE_TYPE,
                &mut size as *mut _ as *mut c_void,
            )
        };
        if ok != 0 {
            Some(size)
        } else {
            None
        }
    }
}
