use gtk::gdk;
use vte::prelude::*;

pub type ThemeOption = (&'static str, &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalPalette {
    background: &'static str,
    foreground: &'static str,
    cursor: &'static str,
    selection_background: &'static str,
    ansi: [&'static str; 16],
}

pub fn native_theme_options() -> &'static [ThemeOption] {
    &[
        ("system", "System"),
        ("nord-dark", "Nord Dark"),
        ("nord-light", "Nord Light"),
        ("tokyo-night-dark", "Tokyo Night Dark"),
        ("tokyo-night-light", "Tokyo Night Light"),
        ("catppuccin-dark", "Catppuccin Dark"),
        ("catppuccin-light", "Catppuccin Light"),
        ("dracula-dark", "Dracula Dark"),
        ("dracula-light", "Dracula Light"),
        ("one-dark", "One Dark"),
        ("one-light", "One Light"),
        ("solarized-dark", "Solarized Dark"),
        ("solarized-light", "Solarized Light"),
        ("rose-pine-dark", "Rose Pine Dark"),
        ("rose-pine-light", "Rose Pine Light"),
        ("gruvbox-dark", "Gruvbox Dark"),
        ("gruvbox-light", "Gruvbox Light"),
        ("github-dark", "GitHub Dark"),
        ("github-light", "GitHub Light"),
        ("kanagawa-dark", "Kanagawa Dark"),
        ("kanagawa-light", "Kanagawa Light"),
    ]
}

pub fn normalized_native_theme_id(theme: &str) -> String {
    if native_theme_options().iter().any(|(id, _)| *id == theme) {
        return theme.to_string();
    }

    match theme.to_ascii_lowercase().as_str() {
        "system" => "system".to_string(),
        "light" => "github-light".to_string(),
        "dark" => "nord-dark".to_string(),
        _ => "nord-dark".to_string(),
    }
}

pub fn theme_color_scheme(theme: &str) -> adw::ColorScheme {
    match theme.to_ascii_lowercase().as_str() {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        "system" => adw::ColorScheme::Default,
        normalized if normalized.ends_with("-light") => adw::ColorScheme::ForceLight,
        normalized if normalized.ends_with("-dark") => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    }
}

pub fn theme_is_dark(theme: &str) -> bool {
    match normalized_native_theme_id(theme).as_str() {
        "system" => adw::StyleManager::default().is_dark(),
        normalized if normalized.ends_with("-light") => false,
        _ => true,
    }
}

pub fn sidebar_theme_css(theme: &str, is_dark: bool) -> String {
    let palette = terminal_palette(theme, is_dark);
    format!(
        ".lantern-sidebar {{ background-color: {}; }} \
         .lantern-sidebar label {{ color: {}; }}",
        palette.background, palette.foreground,
    )
}

pub fn apply_terminal_theme(terminal: &vte::Terminal, theme: &str, is_dark: bool) {
    let palette = terminal_palette(theme, is_dark);
    let foreground = parse_rgba(palette.foreground);
    let background = parse_rgba(palette.background);
    let cursor = parse_rgba(palette.cursor);
    let selection_background = parse_rgba(palette.selection_background);
    let ansi = palette.ansi.map(parse_rgba);
    let ansi_refs = ansi.iter().collect::<Vec<_>>();

    terminal.set_colors(Some(&foreground), Some(&background), &ansi_refs);
    terminal.set_color_cursor(Some(&cursor));
    terminal.set_color_cursor_foreground(Some(&background));
    terminal.set_color_highlight(Some(&selection_background));
}

fn parse_rgba(value: &str) -> gdk::RGBA {
    gdk::RGBA::parse(value).expect("native terminal palette contains valid RGBA colors")
}

fn resolved_terminal_theme_id(theme: &str, prefers_dark: bool) -> &'static str {
    match normalized_native_theme_id(theme).as_str() {
        "system" => {
            if prefers_dark {
                "nord-dark"
            } else {
                "github-light"
            }
        }
        "nord-dark" => "nord-dark",
        "nord-light" => "nord-light",
        "tokyo-night-dark" => "tokyo-night-dark",
        "tokyo-night-light" => "tokyo-night-light",
        "catppuccin-dark" => "catppuccin-dark",
        "catppuccin-light" => "catppuccin-light",
        "dracula-dark" => "dracula-dark",
        "dracula-light" => "dracula-light",
        "one-dark" => "one-dark",
        "one-light" => "one-light",
        "solarized-dark" => "solarized-dark",
        "solarized-light" => "solarized-light",
        "rose-pine-dark" => "rose-pine-dark",
        "rose-pine-light" => "rose-pine-light",
        "gruvbox-dark" => "gruvbox-dark",
        "gruvbox-light" => "gruvbox-light",
        "github-dark" => "github-dark",
        "github-light" => "github-light",
        "kanagawa-dark" => "kanagawa-dark",
        "kanagawa-light" => "kanagawa-light",
        _ => "nord-dark",
    }
}

fn terminal_palette(theme: &str, prefers_dark: bool) -> TerminalPalette {
    match resolved_terminal_theme_id(theme, prefers_dark) {
        "nord-dark" => TerminalPalette {
            background: "#2e3440",
            foreground: "#eceff4",
            cursor: "#88c0d0",
            selection_background: "rgba(136, 192, 208, 0.2)",
            ansi: [
                "#3b4252", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#88c0d0",
                "#e5e9f0", "#4c566a", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead",
                "#8fbcbb", "#eceff4",
            ],
        },
        "nord-light" => TerminalPalette {
            background: "#eceff4",
            foreground: "#2e3440",
            cursor: "#5e81ac",
            selection_background: "rgba(94, 129, 172, 0.2)",
            ansi: [
                "#2e3440", "#bf616a", "#a3be8c", "#d08770", "#5e81ac", "#b48ead", "#88c0d0",
                "#e5e9f0", "#4c566a", "#bf616a", "#a3be8c", "#d08770", "#81a1c1", "#b48ead",
                "#8fbcbb", "#eceff4",
            ],
        },
        "tokyo-night-dark" => TerminalPalette {
            background: "#1a1b26",
            foreground: "#c0caf5",
            cursor: "#7aa2f7",
            selection_background: "rgba(122, 162, 247, 0.2)",
            ansi: [
                "#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff",
                "#a9b1d6", "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7",
                "#7dcfff", "#c0caf5",
            ],
        },
        "tokyo-night-light" => TerminalPalette {
            background: "#d5d6db",
            foreground: "#343b58",
            cursor: "#34548a",
            selection_background: "rgba(52, 84, 138, 0.2)",
            ansi: [
                "#0f0f14", "#8c4351", "#485e30", "#8f5e15", "#34548a", "#5a4a78", "#0f4b6e",
                "#343b58", "#9699a3", "#8c4351", "#485e30", "#8f5e15", "#34548a", "#5a4a78",
                "#0f4b6e", "#343b58",
            ],
        },
        "catppuccin-dark" => TerminalPalette {
            background: "#1e1e2e",
            foreground: "#cdd6f4",
            cursor: "#89b4fa",
            selection_background: "rgba(137, 180, 250, 0.2)",
            ansi: [
                "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#cba6f7", "#94e2d5",
                "#bac2de", "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#cba6f7",
                "#94e2d5", "#a6adc8",
            ],
        },
        "catppuccin-light" => TerminalPalette {
            background: "#eff1f5",
            foreground: "#4c4f69",
            cursor: "#1e66f5",
            selection_background: "rgba(30, 102, 245, 0.15)",
            ansi: [
                "#5c5f77", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#8839ef", "#179299",
                "#acb0be", "#6c6f85", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#8839ef",
                "#179299", "#bcc0cc",
            ],
        },
        "dracula-dark" => TerminalPalette {
            background: "#282a36",
            foreground: "#f8f8f2",
            cursor: "#bd93f9",
            selection_background: "rgba(189, 147, 249, 0.25)",
            ansi: [
                "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd",
                "#f8f8f2", "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5", "#d6acff", "#ff92df",
                "#a4ffff", "#ffffff",
            ],
        },
        "dracula-light" => TerminalPalette {
            background: "#f8f8f2",
            foreground: "#282a36",
            cursor: "#7c3aed",
            selection_background: "rgba(124, 58, 237, 0.15)",
            ansi: [
                "#282a36", "#d73a49", "#22863a", "#b08800", "#7c3aed", "#d63384", "#0d9ea3",
                "#f8f8f2", "#6272a4", "#d73a49", "#22863a", "#b08800", "#7c3aed", "#d63384",
                "#0d9ea3", "#ffffff",
            ],
        },
        "one-dark" => TerminalPalette {
            background: "#282c34",
            foreground: "#abb2bf",
            cursor: "#61afef",
            selection_background: "rgba(97, 175, 239, 0.2)",
            ansi: [
                "#3f4451", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2",
                "#abb2bf", "#5c6370", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd",
                "#56b6c2", "#ffffff",
            ],
        },
        "one-light" => TerminalPalette {
            background: "#fafafa",
            foreground: "#383a42",
            cursor: "#4078f2",
            selection_background: "rgba(64, 120, 242, 0.15)",
            ansi: [
                "#383a42", "#e45649", "#50a14f", "#c18401", "#4078f2", "#a626a4", "#0184bc",
                "#a0a1a7", "#696c77", "#e45649", "#50a14f", "#c18401", "#4078f2", "#a626a4",
                "#0184bc", "#fafafa",
            ],
        },
        "solarized-dark" => TerminalPalette {
            background: "#002b36",
            foreground: "#839496",
            cursor: "#268bd2",
            selection_background: "rgba(38, 139, 210, 0.2)",
            ansi: [
                "#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198",
                "#eee8d5", "#586e75", "#cb4b16", "#859900", "#b58900", "#268bd2", "#6c71c4",
                "#2aa198", "#fdf6e3",
            ],
        },
        "solarized-light" => TerminalPalette {
            background: "#fdf6e3",
            foreground: "#657b83",
            cursor: "#268bd2",
            selection_background: "rgba(38, 139, 210, 0.15)",
            ansi: [
                "#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198",
                "#eee8d5", "#586e75", "#cb4b16", "#859900", "#b58900", "#268bd2", "#6c71c4",
                "#2aa198", "#fdf6e3",
            ],
        },
        "rose-pine-dark" => TerminalPalette {
            background: "#191724",
            foreground: "#e0def4",
            cursor: "#c4a7e7",
            selection_background: "rgba(196, 167, 231, 0.2)",
            ansi: [
                "#26233a", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7", "#ebbcba",
                "#e0def4", "#6e6a86", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7",
                "#ebbcba", "#e0def4",
            ],
        },
        "rose-pine-light" => TerminalPalette {
            background: "#faf4ed",
            foreground: "#575279",
            cursor: "#907aa9",
            selection_background: "rgba(144, 122, 169, 0.15)",
            ansi: [
                "#575279", "#b4637a", "#56949f", "#ea9d34", "#286983", "#907aa9", "#d7827e",
                "#f2e9de", "#9893a5", "#b4637a", "#56949f", "#ea9d34", "#286983", "#907aa9",
                "#d7827e", "#faf4ed",
            ],
        },
        "gruvbox-dark" => TerminalPalette {
            background: "#282828",
            foreground: "#ebdbb2",
            cursor: "#fe8019",
            selection_background: "rgba(254, 128, 25, 0.2)",
            ansi: [
                "#282828", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a",
                "#a89984", "#928374", "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#d3869b",
                "#8ec07c", "#ebdbb2",
            ],
        },
        "gruvbox-light" => TerminalPalette {
            background: "#fbf1c7",
            foreground: "#3c3836",
            cursor: "#af3a03",
            selection_background: "rgba(175, 58, 3, 0.15)",
            ansi: [
                "#3c3836", "#9d0006", "#79740e", "#b57614", "#076678", "#8f3f71", "#427b58",
                "#a89984", "#928374", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286",
                "#689d6a", "#fbf1c7",
            ],
        },
        "github-dark" => TerminalPalette {
            background: "#0d1117",
            foreground: "#c9d1d9",
            cursor: "#58a6ff",
            selection_background: "rgba(88, 166, 255, 0.2)",
            ansi: [
                "#484f58", "#ff7b72", "#3fb950", "#d29922", "#58a6ff", "#bc8cff", "#39c5cf",
                "#b1bac4", "#6e7681", "#ffa198", "#56d364", "#e3b341", "#79c0ff", "#d2a8ff",
                "#56d4dd", "#f0f6fc",
            ],
        },
        "github-light" => TerminalPalette {
            background: "#ffffff",
            foreground: "#1f2328",
            cursor: "#0969da",
            selection_background: "rgba(9, 105, 218, 0.15)",
            ansi: [
                "#24292f", "#cf222e", "#1a7f37", "#9a6700", "#0969da", "#8250df", "#1b7c83",
                "#6e7781", "#57606a", "#a40e26", "#2da44e", "#bf8700", "#218bff", "#a475f9",
                "#3192aa", "#8c959f",
            ],
        },
        "kanagawa-dark" => TerminalPalette {
            background: "#1f1f28",
            foreground: "#dcd7ba",
            cursor: "#7e9cd8",
            selection_background: "rgba(126, 156, 216, 0.2)",
            ansi: [
                "#16161d", "#c34043", "#76946a", "#c0a36e", "#7e9cd8", "#957fb8", "#6a9589",
                "#c8c093", "#727169", "#e82424", "#98bb6c", "#e6c384", "#7fb4ca", "#938aa9",
                "#7aa89f", "#dcd7ba",
            ],
        },
        "kanagawa-light" => TerminalPalette {
            background: "#f2ecbc",
            foreground: "#545453",
            cursor: "#4d699b",
            selection_background: "rgba(77, 105, 155, 0.15)",
            ansi: [
                "#545453", "#c84053", "#6f894e", "#77713f", "#4d699b", "#624c83", "#597b75",
                "#c8c093", "#a0a0a0", "#d7474b", "#6e915f", "#836f4a", "#6693bf", "#7e5a9b",
                "#5e857a", "#f2ecbc",
            ],
        },
        _ => unreachable!("unknown native terminal theme palette"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_native_theme_id_keeps_system_theme() {
        assert_eq!(normalized_native_theme_id("system"), "system".to_string());
    }

    #[test]
    fn resolved_terminal_theme_id_uses_system_dark_default() {
        assert_eq!(resolved_terminal_theme_id("system", true), "nord-dark");
    }

    #[test]
    fn resolved_terminal_theme_id_uses_system_light_default() {
        assert_eq!(resolved_terminal_theme_id("system", false), "github-light");
    }

    #[test]
    fn resolved_terminal_theme_id_falls_back_to_nord_dark() {
        assert_eq!(
            resolved_terminal_theme_id("definitely-not-a-theme", true),
            "nord-dark"
        );
    }

    #[test]
    fn terminal_palette_uses_theme_background_and_foreground() {
        assert_eq!(
            terminal_palette("tokyo-night-dark", true),
            TerminalPalette {
                background: "#1a1b26",
                foreground: "#c0caf5",
                cursor: "#7aa2f7",
                selection_background: "rgba(122, 162, 247, 0.2)",
                ansi: [
                    "#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff",
                    "#a9b1d6", "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7",
                    "#7dcfff", "#c0caf5",
                ],
            }
        );
    }
}
