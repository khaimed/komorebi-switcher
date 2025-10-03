<p align="center"><img src="./assets/icon.svg" width="125" /></p>

# glazewm-switcher

A minimal workspace switcher for Windows taskbars, tailored for [GlazeWM](https://github.com/glzr-io/glazewm).

Fork note: This distribution is maintained by `khaimed`. Original project and much of the groundwork were created by Amr Bashir (`amrbashir`). This fork focuses on a clean GlazeWM-only experience.

![Image showcasing komorebi switcher in Windows 11 dark mode](.github/image-1.jpg)
![Image showcasing komorebi switcher in Windows 11 light mode](.github/image-2.jpg)

## Install

<a href="https://github.com/khaimed/komorebi-switcher/releases">
  <picture>
    <img alt="Get it on GitHub" src="https://github.com/LawnchairLauncher/lawnchair/blob/7336b4a0481406ff9ddd3f6c95ea05830890b1dc/docs/assets/badge-github.png" height="60">
  </picture>
</a>

Or through PowerShell:

```powershell
irm "https://github.com/khaimed/glazewm-switcher/releases/latest/download/glazewm-switcher-setup.exe" -OutFile "glazewm-switcher-setup.exe"
& "./glazewm-switcher-setup.exe"
```

## Usage

- <kbd>Left Click</kbd> any workspace to switch to it.
- <kbd>Right Click</kbd> to open the context menu:

  - **Move & Resize**: Open the move and resize dialog.

    ![430847504-b1839a40-df9e-4685-aaeb-07410e9c379c](https://github.com/user-attachments/assets/20becf18-7e0c-4b9f-9de6-11ac79ef8408)

  - **Quit**: close the switcher

> [!TIP]
> You can also open the context menu from the tray icon.

## Development

1. Install [Rust](https://rustup.rs/)
2. Run `cargo run`

## LICENSE

[MIT](./LICENSE) License
