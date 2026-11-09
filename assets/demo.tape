# VHS Tape: TorqKage interactive terminal control deck
# Output configuration
Output "assets/demo.gif"

# Terminal window setup
Set FontSize 14
Set FontFamily "JetBrainsMono Nerd Font Mono"
Set LetterSpacing 0
Set LineHeight 1.15
Set Width 1200
Set Height 720
Set Padding 20
Set WindowBar Colorful
Set TypingSpeed 40ms

# High-contrast vibrant palette matching Ghostty
Set Theme { "background": "#000000", "foreground": "#ffffff", "selectionBackground": "#264f78", "cursor": "#ffffff", "black": "#000000", "red": "#cd3131", "green": "#00ff00", "yellow": "#e5e510", "blue": "#2472c8", "magenta": "#bc3fbc", "cyan": "#00ffff", "white": "#e5e5e5", "brightBlack": "#666666", "brightRed": "#f14c4c", "brightGreen": "#00ff00", "brightYellow": "#ffff00", "brightBlue": "#3b8eea", "brightMagenta": "#d670d6", "brightCyan": "#00ffff", "brightWhite": "#ffffff" }

# Boot environment
Hide
Type "export TERM=xterm-256color" Enter
Type "export PATH=$HOME/projects/rust/torqkage/target/release:$PATH" Enter
Type "cd $HOME/projects/rust/torqkage" Enter
Type "clear" Enter
Show

# 1. Launch tkg control deck
Sleep 500ms
Type "tkg"
Sleep 400ms
Enter
Sleep 2s

# 2. Toggle outer-loop Bubblewrap sandbox isolation
Type "b"
Sleep 1.5s

# 3. Toggle isolated workspace (.tkg/workspaces/<agent>)
Type "w"
Sleep 1.5s

# 4. Cycle agent permission policy (Autonomous -> Supervised -> Planning)
Type "p"
Sleep 1.5s
Type "p"
Sleep 1.5s
Type "p"
Sleep 1.2s

# 5. Reset failure circuit breaker strikes
Type "c"
Sleep 1.5s

# 6. Browse fleet agents
Down
Sleep 1s
Down
Sleep 1s
Down
Sleep 1.2s

# 7. Focus MCP tool matrix & toggle tool state
Right
Sleep 1s
Down
Sleep 800ms
Space
Sleep 1.5s

# 8. Set tool permission gate to ASK (Interactive confirmation)
Type "g"
Sleep 2s

# 9. Atomic rollback / undo unsaved changes
Type "u"
Sleep 2s

# 10. Run in-deck plugin validation & environment health diagnostics
Type "i"
Sleep 3s
Escape
Sleep 1.5s

# 11. Switch to Tool Catalog tab & scroll through the extensive MCP tool library
Tab
Sleep 1.5s
Down
Sleep 600ms
Down
Sleep 600ms
PageDown
Sleep 1.5s
PageDown
Sleep 1.5s
PageUp
Sleep 1.2s

# 12. Switch to Directives Editor
Tab
Sleep 2s

# 13. Open integrated manual & telemetry
Type "?"
Sleep 2.5s
Escape
Sleep 1s

# 14. Exit cleanly
Type "q"
Sleep 1s
