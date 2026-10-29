# Install and open Clay Form

[Documentation home](../README.md#documentation) · [Make your first sculpt](guide/getting-started.md)

Use this guide to build and open Clay Form from its source code. You will need a terminal for setup; once the app opens, you can follow the sculpting guides with a mouse or pen.

## Before you start

Alpha 1 packages are being prepared. This guide covers the current source checkout, rather than assuming a packaged download is available. Platform packaging and real-device validation still need release review.

You need an internet connection for the first build and enough time for Rust and the app's dependencies to download and compile.

| Requirement | What to check |
| --- | --- |
| Rust | Install through [rustup](https://rustup.rs/). Follow its instructions for your operating system, including any compiler or build tools it requests. |
| Git | Install [Git](https://git-scm.com/downloads) to download the source with the command below. |
| Linux desktop | Use a working Wayland or X11 desktop. |
| Linux graphics | Use a Vulkan-capable GPU and driver. Mesa lavapipe can also render in software. |

On Debian or Ubuntu, the Linux build also needs the Wayland and keyboard development libraries used by CI:

```sh
sudo apt-get update
sudo apt-get install --yes libwayland-dev libxkbcommon-dev
```

Other Linux distributions use different package names. This is not a complete driver installation guide; if graphics setup fails, see the [GPU setup notes](gpu-rendering.md#validation).

## Get the source and start the app

1. Install Rust and Git using the links above. If an installer asks you to restart your terminal, do so before continuing.
2. Download the repository and open its folder:

   ```sh
   git clone https://github.com/dustyghost/clay-form.git
   cd clay-form
   ```

   Before the repository becomes public, this step requires GitHub access to it.

3. From that folder, build and run Clay Form:

   ```sh
   cargo run --release
   ```

   Rustup selects the compiler version in `rust-toolchain.toml`. The first build downloads dependencies; later builds reuse them. The pinned compiler and committed `Cargo.lock` keep builds reproducible.

4. Wait for the build to finish. The Clay Form window should open. Continue with [Make your first sculpt](guide/getting-started.md) to shape and save a small moon.

To open Clay Form again, return to the same folder and run `cargo run --release`.

## If the app does not open

| What happens | Next step |
| --- | --- |
| The terminal cannot find `cargo` | Finish the rustup installation, restart the terminal and try again. |
| Downloading the source fails | Check the repository URL and your GitHub access. |
| The build reports missing tools or libraries | Follow the Rust installer guidance for your operating system; on Linux, check the build libraries above. |
| The app reports a graphics error | Check your graphics driver and, on Linux, Vulkan support. See the [GPU setup notes](gpu-rendering.md#validation). |

If you need help, keep the terminal error text and note your operating system and graphics hardware. These details make a setup problem easier to investigate.

For contributor checks and screenshot tools, see [Development](development.md).
