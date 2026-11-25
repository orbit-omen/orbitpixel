# OrbitPixel Browser

> **The web, without leaving your terminal.**

**OrbitPixel Browser** is a standalone, terminal-native web browser written in Rust by **OrbitOmen**. Instead of wrapping Chromium, Electron, or a WebView, OrbitPixel implements its own async networking, HTML5 parser, arena DOM, layout pipeline, and terminal renderer.

---

## Quick Install

### Linux & macOS (`sh` / `bash` / `zsh`)
```sh
curl -fsSL https://raw.githubusercontent.com/orbit-omen/orbitpixel/main/install.sh | sh
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/orbit-omen/orbitpixel/main/install.ps1 | iex
```

> **Note:** The installer fetches the latest prebuilt binary from GitHub Releases and verifies its SHA-256 checksum. If no release binary exists yet for your platform, it automatically falls back to `cargo install --git https://github.com/orbit-omen/orbitpixel --branch main`.

---

## Usage

```sh
# Launch OrbitPixel on the built-in start page (default: Color mode)
orbitpixel

# Open a URL directly
orbitpixel https://example.com

# Choose a rendering mode explicitly
orbitpixel text https://example.com
orbitpixel color https://example.com
orbitpixel pixel https://example.com
```

---

## Rendering Modes

| Mode | Command | Description |
| :--- | :--- | :--- |
| **Text** | `orbitpixel text <url>` | Clean plain-text rendering of headings, paragraphs, lists, links, and code blocks. |
| **Color** | `orbitpixel color <url>` | ANSI foreground/background styling, colored headings and links, borders, and quote rails. |
| **Pixel** | `orbitpixel pixel <url>` | Graphics-capable mode designed for inline terminal images (Kitty, Sixel, iTerm2, Unicode half-blocks). |

Press **`m`** in Normal mode at any time to cycle live between `TEXT`, `COLOR`, and `PIXEL` modes.

---

## Keyboard & Mouse Controls

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + L` or `i` | Focus URL bar (`InsertUrl` mode) |
| `Enter` | Navigate to entered URL or open focused link |
| `Esc` | Cancel URL editing and return to `Normal` mode |
| `Tab` / `Shift + Tab` | Cycle forward / backward through links on the page |
| `j` / `k` or `Up` / `Down` | Scroll down / up one line |
| `Space` / `PageDown` / `PageUp` | Scroll by viewport page |
| `g` / `G` or `Home` / `End` | Jump to top / bottom of page |
| `Alt + Left` / `Alt + Right` | Navigate back / forward in tab history |
| `Ctrl + R` | Reload current page |
| `Ctrl + T` | Open new tab |
| `Ctrl + W` | Close active tab |
| `Ctrl + Tab` / `Ctrl + Shift + Tab` | Switch to next / previous tab |
| `m` | Cycle rendering mode (`TEXT` → `COLOR` → `PIXEL`) |
| `q` or `Ctrl + C` | Quit OrbitPixel |
| Mouse Wheel / Left Click | Scroll viewport and click hyperlinks |

---

## Architecture

```text
ORBITPIXEL BROWSER
       │
  Ratatui TUI (Tabs, Address Bar, Viewport, Status Bar)
       │
   App State & Input Handler (Normal / InsertUrl)
       │
 Async HTTP/HTTPS Engine (Tokio + Reqwest + Rustls)
       │
 HTML5 Tokenizer & Arena DOM (Vec<Node> + NodeId)
       │
 Terminal Renderer (Line Wrapping + Link Hit-Testing)
```

---

## Building from Source

Requires Rust 1.75+ (`https://rustup.rs`):

```sh
git clone https://github.com/orbit-omen/orbitpixel.git
cd orbitpixel
cargo build --release
./target/release/orbitpixel https://example.com
```

---

## Security Defaults

- Strict `http://` and `https://` scheme allowlist (blocks `file://`, `javascript:`, etc.).
- TLS certificate verification via `rustls`.
- Bounded HTTP redirects (max 10), request timeouts (15s), and maximum response payload size (5 MB).

---

## License

MIT License — see [LICENSE](LICENSE).
