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
    #[allow(unused)]
    pub device: ID3D11Device,
    // pub desktop: IDCompositionDesktopDevice,
    #[allow(unused)]
    // pub desktop_target: IDCompositionTarget,
    // pub root_visual: IDCompositionVisual2,
    pub width: u32,
    pub height: u32,
    pub render_target: ID2D1DeviceContext,
    pub swapchain: IDXGISwapChain1,
}

impl Dx12Surface {
    pub fn new(window: &Window) -> anyhow::Result<Self> {
        let (width, height) = window.inner_size().into();

        let factory = create_factory()?;
        let device = create_device()?;
        let target = create_render_target(&factory, &device)?;
        let hwnd = window.window_handle()?;
        let RawWindowHandle::Win32(hwnd) = hwnd.as_raw() else {
            anyhow::bail!("Window handle must be win32");
        };
        let hwnd = HWND(hwnd.hwnd.get() as _);
        let swapchain = create_swapchain(&device, hwnd)?;
        create_swapchain_bitmap(&swapchain, &target)?;

        let render_target = create_render_target(&factory, &device)?;

        // let dxgi3: IDXGIDevice3 = device.cast()?;
        // let device_2d = unsafe { D2D1CreateDevice(&dxgi3, None) }?;

        // let desktop: IDCompositionDesktopDevice = unsafe { DCompositionCreateDevice2(&device_2d)? };

        // let desktop_target = unsafe { desktop.CreateTargetForHwnd(hwnd, true) }?;

        // let root_visual = unsafe { desktop.CreateVisual() }?;
        // unsafe { desktop_target.SetRoot(&root_visual) }?;

        // unsafe { root_visual.SetContent(&render_target)? };

        // unsafe { desktop.Commit() }?;

        Ok(Self {
            // desktop,
            // desktop_target,
            // root_visual,
            device,
            render_target,
            height,
            width,
            swapchain,
        })
    }

    pub fn configure(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        Ok(())
    }
}

fn create_factory() -> Result<ID2D1Factory1> {
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

fn get_dxgi_factory(device: &ID3D11Device) -> Result<IDXGIFactory2> {
    let dxdevice = device.cast::<IDXGIDevice>()?;
    unsafe { dxdevice.GetAdapter()?.GetParent() }
}

fn create_swapchain_bitmap(swapchain: &IDXGISwapChain1, target: &ID2D1DeviceContext) -> Result<()> {
    let surface: IDXGISurface = unsafe { swapchain.GetBuffer(0)? };

    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };

    unsafe {
        let bitmap = target.CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
        target.SetTarget(&bitmap);
    };

    Ok(())
}

fn create_swapchain(device: &ID3D11Device, window: HWND) -> Result<IDXGISwapChain1> {
    let factory = get_dxgi_factory(device)?;

    let props = DXGI_SWAP_CHAIN_DESC1 {
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        ..Default::default()
    };

    unsafe { factory.CreateSwapChainForHwnd(device, window, &props, None, None) }
}
