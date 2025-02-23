use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use winit::window::Window;

pub struct Dx12Surface {
    pub width: u32,
    pub height: u32,
    pub hwnd: HWND,
    pub d3d_device: ID3D11Device,
    pub dx_factory: IDXGIFactory2,
    pub swapchain: IDXGISwapChain1,
    pub d2_factory: ID2D1Factory2,
    pub d2_device: ID2D1Device1,
    pub dc: ID2D1DeviceContext1,
    pub surface: IDXGISurface2,
    pub bitmap: ID2D1Bitmap1,
    pub dcomp_device: IDCompositionDevice,
    pub target: IDCompositionTarget,
    pub visual: IDCompositionVisual,
}

impl Dx12Surface {
    pub fn new(window: &Window) -> anyhow::Result<Self> {
        let (width, height) = window.inner_size().into();
        let hwnd = get_hwnd(window)?;
        let d3d_device = create_device()?;
        let dx_factory = create_factory()?;
        let dxgi_device = d3d_device.cast::<IDXGIDevice>()?;
        let swapchain = create_swapchain(&dx_factory, &dxgi_device, width, height)?;
        let d2_factory = create_d2_factory()?;
        let d2_device = unsafe { d2_factory.CreateDevice(&dxgi_device) }?;
        let dc = unsafe { d2_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE) }?;
        let surface: IDXGISurface2 = unsafe { swapchain.GetBuffer(0) }?;
        let bitmap = create_swapchain_bitmap(&dc, &surface)?;
        unsafe { dc.SetTarget(&bitmap) };
        let dcomp_device = create_dcomp_device(&dxgi_device)?;
        let target = unsafe { dcomp_device.CreateTargetForHwnd(hwnd, true) }?;
        let visual = unsafe { dcomp_device.CreateVisual() }?;
        unsafe { visual.SetContent(&swapchain) }?;
        unsafe { target.SetRoot(&visual) }?;
        unsafe { dcomp_device.Commit() }?;

        Ok(Self {
            width,
            height,
            hwnd,
            d3d_device,
            dx_factory,
            swapchain,
            d2_factory,
            d2_device,
            dc,
            surface,
            bitmap,
            dcomp_device,
            target,
            visual,
        })
    }

    pub fn configure(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        Ok(())
    }
}

fn get_hwnd(window: &Window) -> anyhow::Result<HWND> {
    let hwnd = window.window_handle()?;
    let RawWindowHandle::Win32(hwnd) = hwnd.as_raw() else {
        anyhow::bail!("Window handle must be win32");
    };
    Ok(HWND(hwnd.hwnd.get() as _))
}

fn create_factory() -> Result<IDXGIFactory2> {
    let mut flags = DXGI_CREATE_FACTORY_FLAGS::default();

    if cfg!(debug_assertions) {
        flags |= DXGI_CREATE_FACTORY_DEBUG;
    }

    unsafe { CreateDXGIFactory2(flags) }
}

fn create_d2_factory() -> Result<ID2D1Factory2> {
    let mut options = D2D1_FACTORY_OPTIONS::default();

    if cfg!(debug_assertions) {
        options.debugLevel = D2D1_DEBUG_LEVEL_INFORMATION;
    }

    unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, Some(&options)) }
}

fn create_device_with_type(drive_type: D3D_DRIVER_TYPE) -> Result<ID3D11Device> {
    let mut flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;

    if cfg!(debug_assertions) {
        flags |= D3D11_CREATE_DEVICE_DEBUG;
    }

    let mut device = None;

    unsafe {
        D3D11CreateDevice(
            None,
            drive_type,
            HMODULE::default(),
            flags,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
        .map(|()| device.unwrap())
    }
}

fn create_device() -> Result<ID3D11Device> {
    let mut result = create_device_with_type(D3D_DRIVER_TYPE_HARDWARE);

    if let Err(err) = &result {
        if err.code() == DXGI_ERROR_UNSUPPORTED {
            result = create_device_with_type(D3D_DRIVER_TYPE_WARP);
        }
    }

    result
}

fn create_render_target(
    factory: &ID2D1Factory1,
    device: &ID3D11Device,
) -> Result<ID2D1DeviceContext> {
    unsafe {
        let d2device = factory.CreateDevice(&device.cast::<IDXGIDevice>()?)?;
        let target = d2device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
        target.SetUnitMode(D2D1_UNIT_MODE_DIPS);
        Ok(target)
    }
}

fn create_swapchain(
    dx_factory: &IDXGIFactory2,
    dxgi_device: &IDXGIDevice,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    let props = DXGI_SWAP_CHAIN_DESC1 {
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        Height: height,
        Width: width,
        ..Default::default()
    };

    unsafe { dx_factory.CreateSwapChainForComposition(dxgi_device, &props, None) }
}

fn create_swapchain_bitmap(
    dc: &ID2D1DeviceContext1,
    surface: &IDXGISurface2,
) -> Result<ID2D1Bitmap1> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };

    unsafe { dc.CreateBitmapFromDxgiSurface(surface, Some(&props)) }
}

fn create_dcomp_device(dxgi_device: &IDXGIDevice) -> Result<IDCompositionDevice> {
    unsafe { DCompositionCreateDevice2(dxgi_device) }
}
