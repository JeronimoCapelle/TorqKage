# VHS Tape: agy-suite interactive terminal control deck
# Output configuration
Output assets/demo.gif
Output assets/demo.mp4

# Terminal window setup
Set FontSize 14
Set FontFamily "JetBrains Mono"
Set Width 1200
Set Height 720
Set Padding 20
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful

# Boot environment
Hide
Type "export TERM=xterm-256color" Enter
Type "cd projects/rust/agy-suite" Enter
Type "clear" Enter
Show

# 1. Launch agy-ctrl control deck
Sleep 500ms
Type "agy-ctrl"
Sleep 500ms
Enter

# 2. Inspect Fleet Matrix & Agent Profiles
Sleep 2s
Down
Sleep 800ms
Down
Sleep 800ms
Down
Sleep 1s

# 3. Toggle tool permissions in the MCP matrix
Right
Sleep 800ms
Down
Sleep 500ms
Space
Sleep 1s
Space
Sleep 800ms
Left
Sleep 800ms

# 4. Switch to Tool Catalog inspector
Tab
Sleep 2s
Down
Sleep 600ms
Down
Sleep 600ms

# 5. Open integrated manual & telemetry
Type "?"
Sleep 2.5s
Escape
Sleep 800ms

# 6. Exit cleanly
Type "q"
Sleep 1s
