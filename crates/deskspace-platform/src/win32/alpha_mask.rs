//! Direct2D's AlphaMask effect described to the compositor, so a space's
//! blur can be masked (ROADMAP D12, TASKS T13): the first source's pixels
//! times the second's alpha. `CompositionMaskBrush` refuses a host backdrop
//! brush as its source (T1), and composition takes effect graphs only
//! through these interfaces, which Win2D usually supplies.
//! https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.effects.interop/nn-windows-graphics-effects-interop-igraphicseffectd2d1interop
//! https://learn.microsoft.com/en-us/windows/win32/direct2d/alpha-mask-effect

use std::cell::RefCell;

use windows::Foundation::IPropertyValue;
use windows::Graphics::Effects::{
    IGraphicsEffect, IGraphicsEffect_Impl, IGraphicsEffectSource, IGraphicsEffectSource_Impl,
};
use windows::UI::Composition::CompositionEffectSourceParameter;
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct2D::CLSID_D2D1AlphaMask;
use windows::Win32::System::WinRT::Graphics::Direct2D::{
    GRAPHICS_EFFECT_PROPERTY_MAPPING, IGraphicsEffectD2D1Interop, IGraphicsEffectD2D1Interop_Impl,
};
use windows::core::{GUID, HSTRING, Interface, PCWSTR, Result};
use windows_core::implement;

/// The effect's source parameter: what is masked.
pub(crate) const SOURCE: &str = "source";
/// The effect's mask parameter: whose alpha shows the source.
pub(crate) const MASK: &str = "mask";

/// The graph, with its two sources named `SOURCE` and `MASK`.
pub(crate) fn effect() -> Result<IGraphicsEffect> {
    let parameter = |name: &str| -> Result<IGraphicsEffectSource> {
        CompositionEffectSourceParameter::Create(&HSTRING::from(name))?.cast()
    };
    Ok(AlphaMask {
        name: RefCell::new(HSTRING::from("AlphaMask")),
        sources: [parameter(SOURCE)?, parameter(MASK)?],
    }
    .into())
}

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct AlphaMask {
    name: RefCell<HSTRING>,
    sources: [IGraphicsEffectSource; 2],
}

impl IGraphicsEffect_Impl for AlphaMask_Impl {
    fn Name(&self) -> Result<HSTRING> {
        Ok(self.name.borrow().clone())
    }

    fn SetName(&self, name: &HSTRING) -> Result<()> {
        *self.name.borrow_mut() = name.clone();
        Ok(())
    }
}

impl IGraphicsEffectSource_Impl for AlphaMask_Impl {}

impl IGraphicsEffectD2D1Interop_Impl for AlphaMask_Impl {
    fn GetEffectId(&self) -> Result<GUID> {
        Ok(CLSID_D2D1AlphaMask)
    }

    // AlphaMask has no properties to animate or map.
    fn GetNamedPropertyMapping(
        &self,
        _name: &PCWSTR,
        _index: *mut u32,
        _mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> Result<()> {
        Err(E_INVALIDARG.into())
    }

    fn GetPropertyCount(&self) -> Result<u32> {
        Ok(0)
    }

    fn GetProperty(&self, _index: u32) -> Result<IPropertyValue> {
        Err(E_INVALIDARG.into())
    }

    fn GetSource(&self, index: u32) -> Result<IGraphicsEffectSource> {
        self.sources
            .get(index as usize)
            .cloned()
            .ok_or_else(|| E_INVALIDARG.into())
    }

    fn GetSourceCount(&self) -> Result<u32> {
        Ok(self.sources.len() as u32)
    }
}
